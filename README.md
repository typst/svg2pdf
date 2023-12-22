# svg2pdf

[![Build status](https://github.com/typst/svg2pdf/workflows/Continuous%20integration/badge.svg)](https://github.com/typst/svg2pdf/actions)
[![Current crates.io release](https://img.shields.io/crates/v/svg2pdf)](https://crates.io/crates/svg2pdf)
[![Documentation](https://img.shields.io/badge/docs.rs-svg2pdf-66c2a5?labelColor=555555&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K)](https://docs.rs/svg2pdf/)

Convert SVG files to PDFs.

This crate allows to convert static (i.e. non-interactive) SVG files to
either standalone PDF files or Form XObjects that can be embedded in another
PDF file and used just like images.

Apart from groups with filters on them, the conversion will translate 
the SVG content to PDF without rasterizing it, so no quality is lost.

## Example

This example reads an SVG file and writes the corresponding PDF back to the disk.

```rust
let path = "tests/svg/custom/integration/matplotlib/time_series.svg";
let svg = std::fs::read_to_string(path)?;

// This can only fail if the SVG is malformed. This one is not.
let pdf = svg2pdf::convert_str(&svg, svg2pdf::Options::default())?;

// ... and now you have a Vec<u8> which you could write to a file or
// transmit over the network!
std::fs::write("target/time_series.pdf", pdf)?;
```

## CLI

This crate also contains a command line interface. Install it by running the
command below:

```bash
cargo install svg2pdf-cli
```

You can then convert SVGs to PDFs by running commands like these:

```bash
svg2pdf your.svg
```

## Supported features
In general, a large part of the SVG specification is supported, including
features like:
- Path drawing with fills and strokes
- Gradients
- Patterns
- Clip paths
- Masks
- Filters
- Transformation matrices
- Respecting the `keepAspectRatio` attribute
- Raster images and nested SVGs

## Unsupported features
Among the unsupported features are currently:
- The `spreadMethod` attribute of gradients
- Text will be converted into shapes before converting to PDF. It is planned
to add support for text preservation in a future update.
- Raster images are not color managed but use PDF's DeviceRGB color space
- A number of features that were added in SVG2 
(see [here](https://github.com/RazrFalcon/resvg/blob/master/docs/svg2-changelog.md))

## Contributing
We are looking forward to receiving your bugs and feature requests in the Issues
tab. We would also be very happy to accept PRs for bug fixes, features, or
refactorings! We'd be happy to assist you
with your PR's, so feel free to post Work in Progress PRs if marked as such.
Please be kind to the maintainers and other contributors. If you feel that there
are any problems, please feel free to reach out to us privately.

Thanks to each and every prospective contributor for the effort you (plan to)
invest in this project and for adopting it!

## License
`svg2pdf` is licensed under a MIT / Apache 2.0 dual license.

Users and consumers of the library may choose which of those licenses they want
to apply whereas contributors have to accept that their code is in compliance
and distributed under the terms of both of these licenses.
