# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.12.0]

### Added
- New `pdfa` option for using PDF/A-compliant pdf-writer workflows.

### Changed
- Conversion is now fallible and returns a `Result`.
- Reduce PDF sizes through better font subsetting.

### Fixed
- Fixed a bug with Unicode CMaps.

## [0.11.0]

### Added
- Text is now embedded as proper text instead of being flattened to paths.
- Made the CLI more flexible in terms of which features you want to include.
- Added `raster-scale` and `text-to-paths` as arguments for the CLI.

### Changed
- Bumped resvg to v0.42, fontdb to v0.18, and pdf-writer to v0.10.
- The `convert_tree` method has been renamed into `to_pdf`.
- The `convert_tree_into` function has been renamed into `to_chunk` and now returns an independent chunk and the object ID of the actual SVG in the chunk.

### Fixed
- Fixed a bug with softmasks on images.

### Removed
- The `convert_str` method has been removed. You should now always convert your SVG string into a `usvg` tree yourself and then call either `to_pdf` or `to_chunk`.
- Removed the option to configure the view box from the API. This might be readded in a later update.

## [0.10.0]

### Added
- Added ability to list the available fonts found by svg2pdf. Thanks to [@rgreinho](https://github.com/rgreinho).
- Added support for filter rendering.
- `usvg` is now reexported to prevent version mismatches.

### Fixed
- Fixed dpi ratio calculation. Thanks to [@Ultraxime](https://github.com/Ultraxime).

### Changed
- Bumped resvg to v0.38 and fontdb to v0.16.
- (Internal) reworked the test suite.
- (Internal) synced test suite with resvg test suite.

[Unreleased]: https://github.com/typst/svg2pdf/compare/v0.10.0...HEAD
[0.10.0]: https://github.com/typst/svg2pdf/compare/v0.9.1...v0.10.0
[0.11.0]: https://github.com/typst/svg2pdf/compare/v0.10.0...v0.11.0
[0.12.0]: https://github.com/typst/svg2pdf/compare/v0.11.0...v0.12.0
