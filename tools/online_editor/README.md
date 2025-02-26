# Slint Online Editor

This directory contains the frontend code for the online code editor
which is hosted in https://slint-ui.com/editor (last stable) and
https://slint-ui.com/snapshots/master/editor (nightly)

To try it out locally type this in this directory:
```sh
npm install
npm start
```

## Documentation

The `index.html` page contains a code editor and every key press reload the preview.
The `preview.html` page contains only the preview and the code must be given via query parameter.

* `?load_url=`  query argument make it possible to load the .slint code directly from an URL.
    If the slint code contains relative path for imports or images, they are loaded relative to
    that slint file. That way it is possible to load  code from github (via raw.githubusercontent)
    or gists.

    Example: this loads the printerdemo.slint file from the github URL
    - https://slint-ui.com/editor?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/printerdemo/ui/printerdemo.slint
    - https://slint-ui.com/editor/preview.html?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/printerdemo/ui/printerdemo.slint

* `?snippet=` query argument, followed by the URL-encoded slint code, will simply load this code
   this is what is used tor the premalink feature

   Example: a simple code with "Hello Slint"
   - https://slint-ui.com/editor/?snippet=_+%3A%3D+Text+%7B+text%3A+%22Hello+Slint%22%3B+%7D
   - https://slint-ui.com/editor/preview.html?snippet=_+%3A%3D+Text+%7B+text%3A+%22Hello+Slint%22%3B+%7D

