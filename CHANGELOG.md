# Changelog

All notable changes to thermview are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.6] - 2026-11-12

### Fixed
- The GitHub button links to alfiethology/thermview, using the repository URL from package metadata.

## [0.1.5] - 2026-11-12

### Added
- A collapsible sidebar with Explorer and image-control activities on a left activity bar.
- A desktop directory tree with expandable folders, folder selection, refresh and file opening, including readable errors for unsupported files.
- Ctrl+B toggles the sidebar; Ctrl+Shift+E opens Explorer.

## [0.1.4] - 2026-11-12

### Added
- FLIR CSQ playback with a video-only timeline and per-frame temperature readings.
- Debian installers with a desktop launcher and CSQ file associations.

### Fixed
- Accept extra zero padding before the JPEG-LS end marker in FLIR recordings, so playback can pass affected frames without discarding their thermal data.
- Use the installed executable's absolute path in the packaged desktop launcher.

## [0.1.3] - 2026-11-01

### Fixed
- Copy and paste work again and links open in the browser, which the egui 0.37 bump had broken. The window renders with wgpu instead of glow.

## [0.1.2] - 2026-10-31

### Changed
- egui and eframe 0.37, which keeps the window running on Wayland when its workspace is hidden.

## [0.1.1] - 2026-10-14

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

[Unreleased]: https://github.com/v0l/thermview/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/v0l/thermview/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/v0l/thermview/releases/tag/v0.1.0
