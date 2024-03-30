# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
- Added support for text embedding.
- The `convert_str` method has been removed. You should now always convert your SVG string into a `usvg` 
 tree yourself.
- The `convert_tree` method has been renamed into `to_pdf`, and now requires you to provide the fontdb
 used for the `usvg` tree.
- `convert_tree_into` has been renamed into `to_chunk` and now returns an independent chunk as well
 as the object ID of the SVG.

- TODO: The CLI options have been (temporarily) removed. They will be readded before the next release.
- TODO: Add tests for CLI and svg options
- TODO: Add CLI option to convert text to paths.
- TODO: Add CI test to test builds with different feature
- TODO: Add text feature?

### Changed
- Bumped resvg to v0.40.
- `convert_str` now requires a `fontdb` as an argument as well.

## [0.10.0]

### Added
- Added ability to list the available fonts found by svg2pdf. Thanks to [@rgreinho](https://github.com/rgreinho).
- Added support for filter rendering.
- `usvg` is now reexported to prevent version mismatches.

### Fixed
- Fixed dpi ratio calculation. Thanks to [@Ultraxime](https://github.com/Ultraxime).

### Changed
- Bumped resvg to v0.38 and fontdb to 0.16.
- (Internal) reworked the test suite.
- (Internal) synced test suite with resvg test suite.

[Unreleased]: https://github.com/typst/svg2pdf/compare/v0.10.0...HEAD
[0.10.0]: https://github.com/typst/svg2pdf/compare/v0.9.1...v0.10.0
