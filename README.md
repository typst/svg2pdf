# svg2pdf

[![Build status](https://github.com/typst/svg2pdf/workflows/Continuous%20integration/badge.svg)](https://github.com/typst/svg2pdf/actions)
[![Current crates.io release](https://img.shields.io/crates/v/svg2pdf)](https://crates.io/crates/svg2pdf)
[![Documentation](https://img.shields.io/badge/docs.rs-svg2pdf-66c2a5?labelColor=555555&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K)](https://docs.rs/svg2pdf/)

Convert SVG files to PDFs.

This crate allows to convert static (i.e. non-interactive) SVG files to
either standalone PDF files or Form XObjects that can be embedded in another
PDF file and used just like images.

The conversion will translate the SVG content to PDF without rasterizing it,
so no quality is lost.

## Example
This example reads an SVG file and writes the corresponding PDF back to the disk.

```rust
let svg = std::fs::read_to_string("tests/example.svg").unwrap();

// This can only fail if the SVG is malformed. This one is not.
let pdf = svg2pdf::convert_str(&svg, svg2pdf::Options::default()).unwrap();

// ... and now you have a Vec<u8> which you could write to a file or
// transmit over the network!
std::fs::write("target/example.pdf", pdf).unwrap();
```

## Supported features
- Path drawing with fills and strokes
- Gradients
- Patterns
- Clip paths
- Masks
- Transformation matrices
- Respecting the `keepAspectRatio` attribute
- Raster images and nested SVGs

Filters are not currently supported and embedded raster images are not color
managed. Instead, they use PDF's `DeviceRGB` color space.

## Contributing

We are looking forward to receiving your bugs and feature requests in the Issues
tab. We would also be very happy to accept PRs for bug fixes, features, or
refactorings!

If you want to contribute but are uncertain where to start, yo could look into
filters like `feBlend` and `feColorMatrix` that can be implemented with
transparency groups and color spaces, respectively. We'd be happy to assist you
with your PR's, so feel free to post Work in Progress PRs if marked as such.

That being said: Please keep it civil and do not be an asshole to anyone around
here. This project strives to adhere to and enforce the Contributor Covenant
Code of Conduct. If you are subject to or witness any abusive behavior or other
breaches of the code of conduct, please report them to the project maintainers.

Thanks to each and every prospective contributor for the effort you (plan to)
invest in this project and for adopting it!

## License

svg2pdf is licensed under a MIT / Apache 2.0 dual license.

Users and consumers of the library may choose which of those licenses they want
to apply whereas contributors have to accept that their code is in compliance
and distributed under the terms of both of these licenses.
