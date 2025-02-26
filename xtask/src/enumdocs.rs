// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use anyhow::Context;
use std::io::Write;

pub fn generate() -> Result<(), Box<dyn std::error::Error>> {
    let root = super::root_dir();

    let mut file = std::fs::File::create(root.join("docs/builtin_enums.md"))
        .context("error creating docs/builtin_enums.md")?;

    file.write_all(
        br#"<!-- Generated with `cargo xtask enumdocs` from internal/commons/enums.rs -->
# Builtin Enums

Enum value can be referenced by using the name of the enum and the name of the value
separated by a dot. (eg: `TextHorizontalAlignment.left`)

The name of the enum can be omitted in bindings of the type of that enum, or if the
return value of a callback is of that enum.

The default value of each enum type is always the first value.
"#,
    )?;

    macro_rules! gen_enums {
        ($( $(#[doc = $enum_doc:literal])* enum $Name:ident { $( $(#[doc = $value_doc:literal])* $Value:ident,)* })*) => {
            $(
                writeln!(file, "## `{}`\n", stringify!($Name))?;
                $(writeln!(file, "{}", $enum_doc)?;)*
                writeln!(file, "")?;
                $(
                    let mut has_val = false;
                    write!(file, "* **`{}`**:", to_kebab_case(stringify!($Value)))?;
                    $(
                        if has_val {
                            write!(file, "\n   ")?;
                        }
                        write!(file, "{}", $value_doc)?;
                        has_val = true;
                    )*
                    writeln!(file, "")?;
                )*
                writeln!(file, "")?;
            )*
        }
    }

    #[allow(unused)] // for 'has_val'
    {
        i_slint_common::for_each_enums!(gen_enums);
    }

    Ok(())
}

/// Convert a ascii pascal case string to kebab case
fn to_kebab_case(str: &str) -> String {
    let mut result = Vec::with_capacity(str.len());
    for x in str.as_bytes() {
        if x.is_ascii_uppercase() {
            if !result.is_empty() {
                result.push(b'-');
            }
            result.push(x.to_ascii_lowercase());
        } else {
            result.push(*x);
        }
    }
    String::from_utf8(result).unwrap()
}
