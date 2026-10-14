# Changelog

All notable changes to thermview are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Windows `.msi` and macOS `.dmg` installers, with the thermview icon.

## [0.1.0] - 2026-10-12

First release, as a desktop app in Rust that also runs in the browser.

### Added
- Reads Infiray `.irg`, Hikmicro JPEGs (Pocket2, SP60), FLIR R-JPEG (E40, T640, AX8, B60 and
  others), FLIR AFF `.img` and DJI R-JPEG (Mavic 2 Enterprise Advanced), told apart from the
  bytes rather than the file name.
- Nine palettes with inversion, a colour bar with draggable window handles, and linear,
  logarithmic or histogram-equalised mapping.
- Hottest and coldest pixel markers, spot measurements you can name, and the temperature under
  the pointer.
- FLIR R-JPEG recalibration: change emissivity, distance, reflected and air temperature and
  humidity, and every pixel is worked out again from the raw counts.
- °C or °F everywhere, 1x to 6x scaling, and the camera's EXIF details and GPS.
- PNG export of the view with its markers and labels, from the app or the command line.
- Drag and drop a file onto the window.

[Unreleased]: https://github.com/v0l/thermview/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/v0l/thermview/releases/tag/v0.1.0
