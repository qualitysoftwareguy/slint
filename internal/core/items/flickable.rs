// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! The implementation details behind the Flickable

// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! The `Flickable` item

use super::{
    Item, ItemConsts, ItemRc, ItemRendererRef, KeyEventResult, PointerEventButton, RenderingResult,
};
use crate::animations::{EasingCurve, Instant};
use crate::graphics::{Point, Rect};
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEvent, MouseEvent,
};
use crate::item_rendering::CachedRenderingData;
use crate::items::{PropertyAnimation, Rectangle};
use crate::layout::{LayoutInfo, Orientation};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowAdapter;
use crate::Coord;
use crate::Property;
use alloc::boxed::Box;
use alloc::rc::Rc;
use const_field_offset::FieldOffsets;
use core::cell::RefCell;
use core::pin::Pin;
use core::time::Duration;
#[allow(unused)]
use euclid::num::Ceil;
use i_slint_core_macros::*;
#[cfg(not(feature = "std"))]
#[allow(unused)]
use num_traits::Float;

/// The implementation of the `Flickable` element
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct Flickable {
    pub x: Property<Coord>,
    pub y: Property<Coord>,
    pub width: Property<Coord>,
    pub height: Property<Coord>,
    pub viewport: Rectangle,
    pub interactive: Property<bool>,
    data: FlickableDataBox,

    /// FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for Flickable {
    fn init(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn geometry(self: Pin<&Self>) -> Rect {
        euclid::rect(self.x(), self.y(), self.width(), self.height())
    }

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        if let Some(pos) = event.position() {
            if pos.x < 0 as _ || pos.y < 0 as _ || pos.x > self.width() || pos.y > self.height() {
                return InputEventFilterResult::Intercept;
            }
        }
        if !self.interactive() && !matches!(event, MouseEvent::Wheel { .. }) {
            return InputEventFilterResult::ForwardAndIgnore;
        }
        self.data.handle_mouse_filter(self, event)
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        if !self.interactive() && !matches!(event, MouseEvent::Wheel { .. }) {
            return InputEventResult::EventIgnored;
        }
        if let Some(pos) = event.position() {
            if matches!(event, MouseEvent::Wheel { .. } | MouseEvent::Pressed { .. })
                && (pos.x < 0 as _
                    || pos.y < 0 as _
                    || pos.x > self.width()
                    || pos.y > self.height())
            {
                return InputEventResult::EventIgnored;
            }
        }

        self.data.handle_mouse(self, event)
    }

    fn key_event(
        self: Pin<&Self>,
        _: &KeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(
        self: Pin<&Self>,
        _: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn render(
        self: Pin<&Self>,
        backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
    ) -> RenderingResult {
        let geometry = self.geometry();
        (*backend).combine_clip(
            euclid::rect(0 as _, 0 as _, geometry.width(), geometry.height()),
            0 as _,
            0 as _,
        );
        RenderingResult::ContinueRenderingChildren
    }
}

impl ItemConsts for Flickable {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

#[repr(C)]
/// Wraps the internal data structure for the Flickable
pub struct FlickableDataBox(core::ptr::NonNull<FlickableData>);

impl Default for FlickableDataBox {
    fn default() -> Self {
        FlickableDataBox(Box::leak(Box::new(FlickableData::default())).into())
    }
}
impl Drop for FlickableDataBox {
    fn drop(&mut self) {
        // Safety: the self.0 was constructed from a Box::leak in FlickableDataBox::default
        drop(unsafe { Box::from_raw(self.0.as_ptr()) });
    }
}

impl core::ops::Deref for FlickableDataBox {
    type Target = FlickableData;
    fn deref(&self) -> &Self::Target {
        // Safety: initialized in FlickableDataBox::default
        unsafe { self.0.as_ref() }
    }
}

/// The distance required before it starts flicking if there is another item intercepting the mouse.
const DISTANCE_THRESHOLD: Coord = 8 as _;
/// Time required before we stop caring about child event if the mouse hasn't been moved
const DURATION_THRESHOLD: Duration = Duration::from_millis(500);

#[derive(Default, Debug)]
struct FlickableDataInner {
    /// The position in which the press was made
    pressed_pos: Point,
    pressed_time: Option<Instant>,
    pressed_viewport_pos: Point,
    /// Set to true if the flickable is flicking and capturing all mouse event, not forwarding back to the children
    capture_events: bool,
}

#[derive(Default, Debug)]
pub struct FlickableData {
    inner: RefCell<FlickableDataInner>,
}

impl FlickableData {
    pub fn handle_mouse_filter(
        &self,
        flick: Pin<&Flickable>,
        event: MouseEvent,
    ) -> InputEventFilterResult {
        let mut inner = self.inner.borrow_mut();
        match event {
            MouseEvent::Pressed { position, button: PointerEventButton::Left } => {
                inner.pressed_pos = position;
                inner.pressed_time = Some(crate::animations::current_tick());
                inner.pressed_viewport_pos = Point::new(
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.x)
                        .apply_pin(flick)
                        .get(),
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.y)
                        .apply_pin(flick)
                        .get(),
                );
                if inner.capture_events {
                    InputEventFilterResult::Intercept
                } else {
                    InputEventFilterResult::DelayForwarding(100)
                }
            }
            MouseEvent::Exit | MouseEvent::Released { button: PointerEventButton::Left, .. } => {
                let was_capturing = inner.capture_events;
                Self::mouse_released(&mut inner, flick, event);
                if was_capturing {
                    InputEventFilterResult::Intercept
                } else {
                    InputEventFilterResult::ForwardEvent
                }
            }
            MouseEvent::Moved { position } => {
                let do_intercept = inner.capture_events
                    || inner.pressed_time.map_or(false, |pressed_time| {
                        if crate::animations::current_tick() - pressed_time > DURATION_THRESHOLD {
                            return false;
                        }
                        let can_move_horiz = (Flickable::FIELD_OFFSETS.viewport
                            + Rectangle::FIELD_OFFSETS.width)
                            .apply_pin(flick)
                            .get()
                            > flick.width();
                        let can_move_vert = (Flickable::FIELD_OFFSETS.viewport
                            + Rectangle::FIELD_OFFSETS.height)
                            .apply_pin(flick)
                            .get()
                            > flick.height();
                        let diff = position - inner.pressed_pos;
                        (can_move_horiz && diff.x.abs() > DISTANCE_THRESHOLD)
                            || (can_move_vert && diff.y.abs() > DISTANCE_THRESHOLD)
                    });
                if do_intercept {
                    InputEventFilterResult::Intercept
                } else if inner.pressed_time.is_some() {
                    InputEventFilterResult::ForwardAndInterceptGrab
                } else {
                    InputEventFilterResult::ForwardEvent
                }
            }
            MouseEvent::Wheel { position, .. } => {
                InputEventFilterResult::InterceptAndDispatch(MouseEvent::Moved { position })
            }
            // Not the left button
            MouseEvent::Pressed { .. } | MouseEvent::Released { .. } => {
                InputEventFilterResult::ForwardAndIgnore
            }
        }
    }

    pub fn handle_mouse(&self, flick: Pin<&Flickable>, event: MouseEvent) -> InputEventResult {
        let mut inner = self.inner.borrow_mut();
        match event {
            MouseEvent::Pressed { .. } => {
                inner.capture_events = true;
                InputEventResult::GrabMouse
            }
            MouseEvent::Exit | MouseEvent::Released { .. } => {
                Self::mouse_released(&mut inner, flick, event);
                InputEventResult::EventAccepted
            }
            MouseEvent::Moved { position } => {
                if inner.pressed_time.is_some() {
                    inner.capture_events = true;
                    let new_pos = ensure_in_bound(
                        flick,
                        inner.pressed_viewport_pos + (position - inner.pressed_pos),
                    );
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.x)
                        .apply_pin(flick)
                        .set(new_pos.x);
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.y)
                        .apply_pin(flick)
                        .set(new_pos.y);
                    InputEventResult::GrabMouse
                } else {
                    inner.capture_events = false;
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::Wheel { delta_x, delta_y, .. } => {
                let old_pos = Point::new(
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.x)
                        .apply_pin(flick)
                        .get(),
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.y)
                        .apply_pin(flick)
                        .get(),
                );
                let new_pos = ensure_in_bound(
                    flick,
                    old_pos + euclid::default::Vector2D::new(delta_x as _, delta_y as _),
                );
                (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.x)
                    .apply_pin(flick)
                    .set(new_pos.x);
                (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.y)
                    .apply_pin(flick)
                    .set(new_pos.y);
                InputEventResult::EventAccepted
            }
        }
    }

    fn mouse_released(inner: &mut FlickableDataInner, flick: Pin<&Flickable>, event: MouseEvent) {
        if let (Some(pressed_time), Some(pos)) = (inner.pressed_time, event.position()) {
            let dist = (pos - inner.pressed_pos).cast::<f32>();

            let millis = (crate::animations::current_tick() - pressed_time).as_millis();
            if dist.square_length() > (DISTANCE_THRESHOLD * DISTANCE_THRESHOLD) as f32 && millis > 1
            {
                let speed = dist / (millis as f32);

                let duration = 250;
                let final_pos = ensure_in_bound(
                    flick,
                    (inner.pressed_viewport_pos.cast() + dist + speed * (duration as f32)).cast(),
                );
                let anim = PropertyAnimation {
                    duration,
                    easing: EasingCurve::CubicBezier([0.0, 0.0, 0.58, 1.0]),
                    ..PropertyAnimation::default()
                };
                (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.x)
                    .apply_pin(flick)
                    .set_animated_value(final_pos.x, anim.clone());
                (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.y)
                    .apply_pin(flick)
                    .set_animated_value(final_pos.y, anim);
            }
        }
        inner.capture_events = false; // FIXME: should only be set to false once the flick animation is over
        inner.pressed_time = None;
    }
}

/// Make sure that the point is within the bounds
fn ensure_in_bound(flick: Pin<&Flickable>, p: Point) -> Point {
    let w = flick.width();
    let h = flick.height();
    let vw =
        (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.width).apply_pin(flick).get();
    let vh = (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.height)
        .apply_pin(flick)
        .get();

    let min = Point::new(w - vw, h - vh);
    let max = Point::new(0 as _, 0 as _);
    p.max(min).min(max)
}

/// # Safety
/// This must be called using a non-null pointer pointing to a chunk of memory big enough to
/// hold a FlickableDataBox
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn slint_flickable_data_init(data: *mut FlickableDataBox) {
    core::ptr::write(data, FlickableDataBox::default());
}

/// # Safety
/// This must be called using a non-null pointer pointing to an initialized FlickableDataBox
#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn slint_flickable_data_free(data: *mut FlickableDataBox) {
    core::ptr::drop_in_place(data);
}
