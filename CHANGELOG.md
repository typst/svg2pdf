# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
