# ThermView

**A thermal imaging analysis tool for thermographers, not just developers.**

ThermView reads, renders and measures radiometric thermal images, natively on the desktop or in the browser through WebAssembly. It supports Infiray `.irg` files, Hikmicro JPEGs (Pocket2, SP60), FLIR R-JPEG files (E40, T640, AX8, B60 and others), FLIR AFF `.img` files and DJI R-JPEG files (Mavic 2 Enterprise Advanced). It is written in Rust with [egui](https://github.com/emilk/egui) and the [egui_bench](https://github.com/v0l/egui_bench) panel theme.

This is a fork of [v0l/thermview](https://github.com/v0l/thermview). Use this fork's installers or source instructions below for its CSQ playback and range-measurement features. Launchers configured for the upstream project may install the original version instead.

**Temperature validation:** temperatures come from embedded thermal data, with colours applied afterward. The [0.1.11 audit](docs/temperature-validation.md) found a maximum software discrepancy of **0.1693°C**. The current source uses the file's atmospheric coefficients and IR-window settings and preserves zero distance/humidity. A [follow-up comparison](docs/temperature-file-coefficients.md) matched all 22 readable cases within **0.00000191°C** against the saved reference rasters. This measures software agreement, not certified physical temperature accuracy.

<p align="center">
  <img src="web/public/favicon.svg" width="64" alt="ThermView icon" />
</p>

---

## Temperature conversion improvements in v0.2.0

FLIR CSQ and radiometric JPEG temperatures now use the calibration and environmental
settings stored in each file, rather than substituting fixed atmospheric settings:

- Read the file's atmospheric alpha 1/2, beta 1/2 and X coefficients, plus IR-window
  temperature and transmission. Retain these settings for each frame and reuse
  them when recalculating temperatures after user adjustments.
- Preserve legitimate **0 m distance** and **0% humidity**, and accept humidity
  stored as either a fraction or a percentage using ExifTool's convention. The
  calibration panel now permits zero distance.
- Report missing or invalid required CameraInfo calibration instead of silently
  substituting generic Planck constants or searching unrelated bytes for values.

Replaying the original validation samples against the saved Thermimage reference
rasters gave the following results:

| Check | v0.1.11 | v0.2.0 source validation |
|---|---|---|
| Largest absolute pixel temperature difference | 0.1693°C | 0.00000191°C |
| Readable cases passing the 0.001°C agreement threshold | 1 of 22 | 22 of 22 |
| Raw sensor-count mismatches | 0 | 0 |

The comparison covers **1,920,000 pixels** across 19 readable sampled CSQ frames,
two JPEG exports from that recording, and one public reference JPEG. Exported
JPEGs duplicate recording content; these are not independent physical measurements.
The recording contains 16,702 frames, of which 20 were sampled. **Frame 5527 still
fails decoding** and has no validated ThermView temperatures.

These results establish agreement with reference software on the tested inputs,
not absolute camera accuracy. Vendor-software and calibrated physical-temperature
comparisons remain outstanding. See the [original audit](docs/temperature-validation.md),
[follow-up report](docs/temperature-file-coefficients.md), and
[per-case results](docs/validation/temperature-file-coefficients-2026-11-15.json).

## Features

| Feature | Description |
|---|---|
| **Multi-format parsing** | Detects IRG, Hikmicro JPEG, FLIR R-JPEG, FLIR AFF and DJI R-JPEG from the bytes. Every parser returns the same `ThermalImage`, so a new format is one parser. |
| **CSQ video (desktop)** | Open FLIR `.csq` sequences, scrub a bottom timeline, step frames and play/pause with live min/max temperatures |
| **9 colour palettes** | Iron (default), Inferno, Jet, Hot, Lava, Plasma, Rainbow, Arctic, Grayscale, with inversion |
| **Colour bar** | A legend painted from the same colour function as the image, with draggable window handles, ticks in the chosen unit and the out-of-window part dimmed |
| **Mappings** | Linear, logarithmic and histogram equalisation (1024-bin CDF, rescaled to the window) |
| **Overscan** | clip, hide, below colour, above colour |
| **Min/max markers** | ▲ hottest and ▼ coldest pixel with live readings |
| **Info overlays** | Emissivity, camera, lens and capture time in the image corners |
| **Spot measurements** | Click to place, rename in the side panel, readings follow recalibration |
| **Pointer readout** | Temperature of the pixel under the pointer |
| **Recalibration** | For FLIR R-JPEG, edit emissivity, distance, reflected and air temperature and humidity, and every pixel is recomputed from the raw counts |
| **Units** | °C or °F everywhere, including the calibration fields |
| **Scaling** | 1x to 6x nearest-neighbour |
| **Image navigation** | Ctrl+scroll zooms around the pointer; wheel/trackpad, Shift+wheel or dragging pans the zoomed image. The view persists across video frames |
| **PNG export** | The full image with every marker and label, independent of viewer zoom; command-line scale remains available |
| **Metadata** | EXIF make, model, lens, software, serial, capture time and GPS |
| **Drag and drop** | Drop a file onto the window, native or web |

## Installation

ThermView targets Linux, Windows and macOS. The `sudo apt install` command is **only for Debian/Ubuntu**; each operating system uses its own installer. Prebuilt installers do not need Rust, CMake, FFmpeg or FLIR software.

Download the appropriate file from **[this fork's GitHub Releases](https://github.com/alfiethology/thermview/releases)**:

| Computer | Installer produced by the release workflow |
|---|---|
| Debian/Ubuntu, Intel or AMD 64-bit | `thermview_VERSION_amd64.deb` |
| Debian/Ubuntu, ARM64 | `thermview_VERSION_arm64.deb` |
| Arch Linux and Arch-based distributions | Use the source-install instructions below; `.deb` is not an Arch package |
| Windows, Intel or AMD 64-bit | `thermview-x86_64-pc-windows-msvc.msi` |
| macOS, Apple silicon (M-series) | `thermview-aarch64-apple-darwin.dmg` |
| macOS, Intel | `thermview-x86_64-apple-darwin.dmg` |

**Installer availability:** merging changes into `master` does not publish downloads. The [release workflow](.github/workflows/release.yml) builds these packages when a version tag such as `v0.1.8` is pushed. A manual workflow run produces downloadable Actions artifacts, but does not publish a GitHub Release. If the release you need is not listed, use the source-install instructions below. Linux has been built and tested in this workspace. A successful Windows build confirms compilation and installer creation, but running the app on a Windows machine still needs verification; macOS needs a completed native build and runtime verification.

### Debian / Ubuntu

From the folder containing the downloaded `.deb`, run:

```bash
sudo apt install ./thermview_0.1.11-1_amd64.deb
```

For ARM64, use the corresponding `_arm64.deb` file. This installs the `thermview` command, an app-menu entry, an icon and CSQ/IRG file associations. Open **ThermView** from the app menu or run:

```bash
thermview
thermview --version
```

Installing a newer `.deb` with the same command upgrades the existing package. Close the app before updating, then reopen it. Remove it with `sudo apt remove thermview`.

If an older launcher-managed installation is also present, `~/.local/bin/thermview` or a personal `~/.local/share/applications/thermview.desktop` can take precedence over the Debian installation. Run `/usr/bin/thermview` to select the Debian binary explicitly. Remove the obsolete installation or update its shortcut to use that path. After removing an old executable, run `hash -r` in an existing Bash terminal or open a new terminal to clear its cached path.

Packages built locally on a newer distribution may require newer system libraries. Release packages are configured to build on Ubuntu 22.04 runners; the dependency requirements are recorded in each `.deb`.

### Arch Linux / Arch-based distributions

**The `.deb` is for Debian-based distributions, not Arch.** Arch uses `pacman` and a different package format and dependency names. This repository does not currently publish an Arch package or AUR recipe. Use the source-install commands below.

On an up-to-date Arch installation, install the build tools and desktop libraries:

```bash
sudo pacman -Syu --needed base-devel rust cmake git pkgconf \
  wayland libxkbcommon libxkbcommon-x11 libx11 libxcb libxcursor libxi libxrandr \
  libglvnd vulkan-icd-loader zenity
```

Then build and install this fork's released version as your normal user:

```bash
cargo install --git https://github.com/alfiethology/thermview.git --tag v0.1.8 --locked thermview
export PATH="$HOME/.cargo/bin:$PATH"
thermview --version
thermview
```

Linux native file dialogs use the desktop's XDG portal (GNOME/KDE/GTK) with Zenity as a fallback.

The first build can take several minutes. The executable is installed in `~/.cargo/bin`, rather than being managed by `pacman`. Keep that directory on your shell's `PATH` for new terminals. This source install does not create an app-menu shortcut. To update, replace the version tag with a newer release and add `--force`; to remove it, run `cargo uninstall thermview`. These commands have not been run in a native Arch environment here.

See the [pacman manual](https://man.archlinux.org/man/pacman.8.en) for Arch's package-management commands. Fedora, openSUSE and other non-Debian distributions should also use their own native packages or build from source with their distribution's dependency names; the `.deb` is not a universal Linux installer.

### Windows

1. Open [this fork's Releases](https://github.com/alfiethology/thermview/releases) and expand **Assets** for the version you want.
2. Download `thermview-x86_64-pc-windows-msvc.msi` (not **Source code**).
3. Double-click it and follow the installer, approving the administrator prompt when requested.
4. Open **ThermView** from the Start menu.

**If the release has no installer attached:** a successful Windows job may already have uploaded one as an Actions artifact even when a different platform prevented release publishing:

1. Sign in to GitHub and open [Actions → release](https://github.com/alfiethology/thermview/actions/workflows/release.yml).
2. Open the run for your version and confirm its Windows build succeeded.
3. Under **Artifacts** on the run's summary page, download `thermview-x86_64-pc-windows-msvc`.
4. Extract that downloaded artifact ZIP, then double-click the `.msi` inside it.

For the initial `v0.1.8` build, the Windows installer was successfully created in [this run](https://github.com/alfiethology/thermview/actions/runs/43262518961); release publishing was skipped after the macOS jobs were cancelled. Actions artifacts expire, so use Release assets when available. GitHub requires a signed-in account with repository read access to [download workflow artifacts](https://docs.github.com/en/actions/managing-workflow-runs/downloading-workflow-artifacts).

Alternatively, from PowerShell in the download folder, open the installer with:

```powershell
msiexec.exe /i ".\thermview-x86_64-pc-windows-msvc.msi"
```

**Opening files:** use **Open** (Ctrl+O) for the standard Windows file dialog, or drag a file into ThermView. **Open folder** in Explorer uses Windows' folder picker. The MSI registers `.csq`, `.irg`, `.seq`, `.img`, `.jpg` and `.jpeg` as supported file types. To double-click-open a file, right-click it → **Open with → Choose another app → ThermView → Always** (wording varies by Windows version). This is a per-file-type Windows preference; installing ThermView does not replace your existing photo viewer or other default apps. Portable copies do not install these associations.

**If the app's icon flashes a console and closes:** install the updated MSI. GUI launches no longer create a separate console. If graphics startup fails before the app opens, it retries using OpenGL. Returned errors and caught panics display a persistent Windows dialog and are written to `%LOCALAPPDATA%\ThermView\startup.log`. Include the dialog text and that log when reporting a failure. This does not diagnose errors that prevent Windows from loading the executable at all, such as a missing runtime DLL.

The [Windows desktop workflow](https://github.com/alfiethology/thermview/actions/workflows/windows-desktop.yml) builds and tests Windows changes on `master`. When a run succeeds, its **thermview-windows-installer** artifact contains the latest MSI even before a version is published to Releases. Compilation and automated checks do not replace testing on the affected Windows machine.

The MSI installs to `C:\Program Files\ThermView` and creates a Start-menu shortcut. It currently does **not** add `thermview` to `PATH`. To launch it or check its version from PowerShell, use:

```powershell
& "$env:ProgramFiles\ThermView\thermview.exe"
& "$env:ProgramFiles\ThermView\thermview.exe" --version
```

Run the newer MSI to upgrade the existing MSI installation. Portable `.zip` downloads are also produced; use either the MSI installation or a portable copy to avoid launching an older copy accidentally. Remove the MSI installation through **Settings → Apps → Installed apps**.

The installer command uses Windows Installer's documented [`msiexec /i` option](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/msiexec).

### macOS

1. Download the `.dmg` matching your Mac: `aarch64-apple-darwin` for Apple silicon or `x86_64-apple-darwin` for Intel. **About This Mac** shows which processor you have.
2. Open the `.dmg` and drag **ThermView.app** onto **Applications**.
3. Eject the disk image and launch **ThermView** from Applications.

To launch it from Terminal:

```bash
open -a ThermView
```

The app bundle currently does **not** install a `thermview` command on `PATH`. Its executable can be used directly:

```bash
/Applications/ThermView.app/Contents/MacOS/thermview --version
```

To update, quit ThermView and replace the existing app in Applications with the newer copy. Remove the installed app by moving `/Applications/ThermView.app` to the Trash.

The workflow currently uses ad-hoc signing rather than Apple Developer ID signing and notarization. If macOS blocks the downloaded app, review [Apple's instructions for opening an app that has not been notarized](https://support.apple.com/102445); the per-app approval is under **System Settings → Privacy & Security → Open Anyway** after attempting to open it.

### Install from source on Linux, Windows or macOS

Install **Rust 1.95 or newer**, **CMake**, and a **C++ compiler** first. The C++ tools build the bundled CharLS JPEG-LS decoder used for CSQ files.

- **Debian/Ubuntu:** install the native build dependencies with `sudo apt install build-essential cmake pkg-config libwayland-dev libxkbcommon-dev`.
- **Arch Linux:** use the `pacman` dependencies and source-install commands in the Arch section above.
- **Windows:** install Visual Studio Build Tools with **Desktop development with C++** and a Windows SDK, plus CMake. Use the Rust MSVC toolchain and a terminal where the C++ tools and CMake are available.
- **macOS:** install Xcode Command Line Tools (`xcode-select --install`) and CMake.

Once those prerequisites are installed, use the same Cargo command on each platform to build and install the current version from this fork:

```sh
cargo install --git https://github.com/alfiethology/thermview.git --branch master --locked thermview
```

This installs the executable in Cargo's bin directory (normally `~/.cargo/bin`, or `%USERPROFILE%\.cargo\bin` on Windows). With that directory on `PATH`, run `thermview` or `thermview --version`. Source installation does not create the desktop shortcuts provided by the installers. To update a source installation, rerun the command with `--force`; to remove it, use `cargo uninstall thermview`. Avoid keeping a Cargo-installed copy alongside a system installer unless you deliberately manage which executable is on `PATH`.

For local development instead:

```bash
git clone https://github.com/alfiethology/thermview.git
cd thermview
cargo run --release                 # open the viewer
cargo run --release -- photo.jpg    # open the viewer with a file
```

The command line can also read and render without a window:

```bash
thermview info photo.jpg
thermview render photo.jpg -o out.png --palette iron --scale 4 --range 20:40 --point 120,160
```

`render` takes `--fahrenheit`, `--mapping linear|log|equalise`, `--overscan clip|hide|below|above`, `--invert`, `--bare` (no overlays) and any number of `--point X,Y`.

### Build a Debian installer locally

On Debian/Ubuntu, install Rust and the packaging dependencies, then run the packaging script from a checkout of this fork:

```bash
sudo apt install build-essential cmake pkg-config libwayland-dev libxkbcommon-dev dpkg-dev python3
packaging/linux/build-deb.sh
```

The `.deb` and its SHA-256 checksum are written to `dist/`. Build natively on x86-64 for `amd64`, or ARM64 for `arm64`. To package an existing release binary, set `THERMVIEW_BINARY=/path/to/thermview` before running the script. Shared-library version requirements are calculated from that binary, so build on the oldest distribution you want to support.

### CSQ videos

Open or drop a `.csq` file in the desktop app. A compact timeline appears at the bottom only for videos. Drag anywhere on its ruler to scrub; use the transport buttons or **Left/Right** to step a frame, **Home/End** to jump to the ends, and **Space** to play or pause. Playback stops at the last frame.

The zoom scroll bar below the ruler shows which part of the whole recording is visible. Drag either end inward to zoom in, outward to zoom out, or drag its middle to pan without moving the playhead. Scroll the mouse wheel over it or use **+ / −** to zoom around the playhead. **Fit** or a double-click on the zoom scroll bar shows the full recording again. The **View** label gives the visible frame range, and scrubbing the ruler selects frames within that range. Playback and frame stepping keep the current frame visible while preserving the zoom level.

Min/max markers and spot readings update for the displayed frame. **Auto range** is off by default. New images/videos start with a **5–40°C** colour window, expanded immediately if the opening frame has colder or hotter pixels. As frames are viewed, the default window expands for new extrema and does not shrink back, keeping colours comparable. Deliberately editing the window or choosing a preset locks your custom bounds; enabling **Auto range** fits each frame instead. Iron is the default palette. Hottest/high readings are red and coldest/low readings blue.

**Ctrl+scroll over the image** zooms around the pointer. Use the scroll wheel or a trackpad to pan, **Shift+wheel** to pan sideways, or drag the zoomed image. **Reset view** fits the whole image again. Image zoom and position persist through playback and scrubbing, so a particular region stays in view. The timeline zoom controls described above are separate and remain available. Opening another file resets the image view.

Playback automatically skips unreadable frames and carries on to the next readable frame. A **skipped** count beside the transport controls shows the affected frame numbers and errors on hover. Playback ends safely if the remaining tail is unreadable. Manually seeking an unreadable frame still reports its error, and range-measurement failures remain separately documented.

Palette, units and measurement points persist while scrubbing. Export saves the full current frame with its frame number in the PNG filename; viewer zoom and panning do not crop or resize the exported image.

Frames are indexed once and decoded on demand. The encoded file stays in memory, but decoded rasters are only retained for the current frame. The playback rate comes from camera metadata when available; otherwise it defaults to an editable 30 fps. The ruler uses frame numbers so it remains precise when recording timing is unavailable. Compressed JPEG-LS CSQ decoding uses bundled [CharLS](https://github.com/team-charls/charls) on desktop; it is currently unavailable in the WebAssembly build. No FFmpeg or FLIR software is required.

### Browse hot spots within the current frame

Use **Next hottest spot** in the right-hand **Frame hot spots** panel to inspect
another hot area without leaving the current frame. For example, a frame whose
maximum is an ear at 38°C can also contain an eye peak at 37.8°C. This control
lets you inspect that second peak before moving to another video frame.

Browsing pauses playback and shows the selected temperature, zero-based pixel
coordinates and spot rank. The selected peak gets a red **SPOT 2**, **SPOT 3**,
etc. marker in the viewer and image exports. **Reset to hottest** returns to the
frame's actual maximum and its **MAX** marker. The frame's true min/max, colour
bar and range-measurement statistics remain unchanged.

Candidates are local temperature maxima. Connected pixels with the same peak
temperature form one plateau, represented by its first pixel in row order.
**Spot spacing** (default **5 sensor pixels**) suppresses nearby lower-ranked
peaks. Reduce it for closely spaced features, or increase it to skip more of a
large hot area. Tied peaks use top-to-bottom, left-to-right order. Invalid pixels
are excluded, and the button stops at the last candidate rather than wrapping.

This is manual inspection, not automatic identification of eyes or ears. Spacing
can hide nearby features, and local-peak browsing does not guarantee the highest
eye temperature across a video. Click the image to measure any particular pixel
when a feature does not appear in the candidate list.

Changing frames, opening another file, recalibrating, or changing spacing resets
browsing to the hottest spot. Palette, temperature units, colour window and zoom
changes preserve the selection. A zoomed view recentres on the selected spot.
Video **Measure → Next hottest** remains a separate control which ranks frames
by their maximum temperature.

### Measure a video range

Select **Measure** in the left activity bar. Scrub to the first frame and press **I** (or **Mark start**), then scrub to the last frame and press **O** (or **Mark end**). Both endpoints are included, and I/O marks appear on the timeline. Marking pauses playback. Press **Analyse range** to process every selected frame; progress and **Cancel** remain available while you use the viewer.

Results show the hottest and coldest temperatures with their frame numbers and zero-based pixel locations, plus **Average max** (the mean of each frame's hottest reading), **Average min** (the mean of each frame's coldest reading), and **Overall mean** (all valid pixel temperatures across all selected frames, weighted equally per pixel). **Go to hottest/coldest** opens the corresponding frame. **Next hottest** steps through distinct frames ranked by each frame's maximum temperature, highest first. **Previous hottest**, beside it, goes back one rank so you can recover after stepping too far. Equal maxima use chronological frame order. The hottest card shows the selected candidate's temperature, pixel location, frame and rank. Next stops at the last candidate and Previous stops at rank 1; neither wraps. **Go to hottest** returns directly to rank 1 and restarts browsing. Previous/Next follow temperature rank, not chronological frame order. Range-wide extrema and averages remain unchanged. Changing the range, recalibrating, or running a new analysis resets candidate browsing. A single-frame range is valid. The time span is the first-to-last frame interval using the current playback frame rate.

Analysis uses each frame's recorded calibration, or your current edited calibration when recalibrated. Changing calibration or either mark clears old results; changing the palette or display unit does not. Invalid temperatures are excluded. Unreadable frames and frames with no valid readings are skipped, and analysis continues through the selected range. The result shows successfully analysed frames versus selected frames. When frames were skipped, a partial-coverage notice makes clear that extrema, averages and ranked candidates describe the readable frames only; the temperatures of skipped frames remain unknown. The **Failures** box at the bottom of Measure lists the skipped frame numbers and errors (hover a row for the full message). If every selected frame fails, no temperature summary is produced. Failure details are cleared with a new analysis, changed marks, recalibration or Reset. **Reset** clears marks and results. I/O shortcuts are ignored while editing text or numeric fields.

### Web

```bash
web/build.sh                        # writes the site to target/web
python3 -m http.server -d target/web 8080
```

The build installs the matching `wasm-bindgen-cli` into `target/tools` on first run, and uses `wasm-opt` when it is on the path. Static files in `web/public` (icons, manifest, `_headers`, `_redirects`) are copied next to `index.html`.

### Tests

```bash
cargo test
```

The tests in `tests/samples.rs` and the FLIR screenshot in `tests/ui.rs` read real camera files from `test-samples/`, which is not tracked, and are skipped when it is missing. `tests/ui.rs` drives the whole app headless and writes screenshots to `target/tmp`.

---

## Usage

The left activity bar switches between **Files** (Explorer), **Look** (palette, range, mapping and display), and **Measure** (video range analysis). Click the active icon again, use **Hide**, or press **Ctrl+B** to collapse the sidebar. **Ctrl+Shift+E** opens Explorer.

On desktop, **Open** and **Open folder** use the operating system's file and folder dialogs, including its drives, recent locations, sidebar and path navigation. The file dialog includes thermal-file filters and **All files**. Explorer shows the current directory, with folders first and expandable subfolders. **Open folder** changes the directory; **Up** goes to its parent; **Refresh** reloads the file list. Click a thermal image or CSQ file to open it. All files are listed; unsupported or unreadable files show an error and keep the current image/video open. Opening a file from outside the current directory switches Explorer to that file's folder. Browser builds retain the activity bar and image controls; filesystem browsing requires the desktop app.

1. Open a file with **open** (ctrl+O) using the native desktop file picker, or drop one onto the window
2. Pick a palette on the left
3. Drag the colour bar handles, type the window, or use a preset
4. Click the image to place measurement points, rename them on the right
5. Switch °C/°F, mapping, overscan, inversion and overlays; use Ctrl+scroll over the image to zoom and the wheel to pan
6. For FLIR R-JPEG, edit the calibration on the right
7. **export** (ctrl+E) saves the full image, independent of viewer zoom, as a PNG next to the source file, or downloads it on the web

---

## File Formats

### FLIR AFF/FFF (binary)

FLIR's proprietary AGEMA File Format used by ThermaCAM and ResearchIR cameras.

| Camera | Resolution | Record Type | Encoding |
|---|---|---|---|
| ThermaCAM PM695 | 327×245 | AFF1 (record type 1) | centi-Kelvin uint16 LE |
| Other SC2000-style | varies | AFF1 | Planck formula |

**File structure:** `AFF\0`/`FFF\0` header → record directory → thermal data record + calibration + raw pixels.

### FLIR R-JPEG

FLIR cameras from the Exx, Txxx, Ax, Bx series embed full radiometric data in standard JPEGs via the APP1 marker. The APP1 contains an FFF binary structure with Planck calibration constants and a PNG-encoded 16-bit raw sensor image.

| Camera | Resolution | Data Location | Encoding |
|---|---|---|---|
| FLIR E40/E60/T640/AX8/B60 | varies by model | APP1 FFF record type 1 | 16-bit PNG (byte-swapped), Planck formula |

**File structure:** JPEG → APP1 chunk(s) with `FLIR\0` header → FFF record directory → RawData PNG + CameraInfo (Planck constants, emissivity, etc.). Large APP1 payloads may be split across multiple chunks. PlanckO and PlanckR2 are read from CameraInfo offsets `0x308` and `0x30c`, the same place exiftool reads them.

### Infiray IRG (binary)

The `.irg` format is produced by Infiray thermal camera modules. The parser auto-detects the camera variant and applies the correct temperature encoding.

**Detected Variants**

| Magic (LE) | Tail (LE) | Device | Temp Encoding |
|---|---|---|---|
| `0xACCA` | `0xCAAC` | C200/C210 | `flag0=0` → K/16, `flag0=1` → K/10 (Autel) |
| `0xBAAB` | `0xCAAC` | Unknown Infiray | K/10 |
| `0x04A0` | — | P200 | K/10 |
| *any* | `0xCAAC` | Unknown CAAC-tail | K/10 (safe default) |

### Hikmicro JPEG

Hikmicro thermal cameras (Pocket2, SP60) embed radiometric data in standard JPEG files.

| Camera | Resolution | Data Offset | Encoding |
|---|---|---|---|
| Pocket2 | 256×192 | HDRI block at offset 44 | uint16 LE centi-°C |
| SP60 | 640×480 | HDRI block at offset 44 | uint16 LE centi-°C |

**File structure:** First JPEG (colorized preview + Iref metadata) → Second JPEG (visible-light) → HDRI block (44-byte header + raw pixel data).

### DJI R-JPEG

DJI drone thermal cameras embed raw sensor data in JPEG APP markers.

| Camera | Resolution | Data Location | Encoding |
|---|---|---|---|
| Mavic 2 Enterprise Advanced | 640×512 | APP3 segments (concatenated) | uint16 LE, raw/64 = K |

**File structure:** APP1 (EXIF) → APP3 × N (raw thermal, uint16 LE) → APP4 (calibration params as float32) → JPEG image data.

The raw/64 conversion is an approximation; DJI's own SDK applies calibration curves that are not public. Files whose APP3 data is not a whole 640×512, 320×256 or 160×120 frame, such as the Zenmuse XT S, are refused rather than shown with made-up temperatures.

### Infiray IRG header fields

| Offset | Size | Field |
|---|---|---|
| `0x22` | 2 bytes | Reference temperature (°C × 1000) |
| `0x24` | 2 bytes | Distance (m × 1000) |
| `0x26` | 2 bytes | Air temperature (°C × 1000) |
| `0x2A` | 2 bytes | Atmospheric transmittance (÷ 10000) |
| `header_len` | W×H bytes | 8-bit coarse (contrast-maximized) image |
| `header_len + W×H` | W×H×2 bytes | 16-bit fine temperature data (raw Kelvin × scale) |
| End | variable | Embedded JPEG thumbnail (optional; consumed as remainder on unknown variants) |

### Temperature Formula

```
°C = raw / tempScale − 273.15
```

Where `tempScale` is 16 for C210 cameras (`flag0 = 0`) or 10 for all other variants. The parser computes the full Celsius grid and min/max/CDF at parse time, so nothing after it touches raw values.

---

## Architecture

```
src/
├── formats/          # one parser per format, plus detection
│   ├── irg.rs        # Infiray IRG
│   ├── hikmicro.rs   # Hikmicro HDRI block
│   ├── dji.rs        # DJI APP3/APP4
│   ├── flir.rs       # FLIR AFF/FFF records
│   └── rjpeg.rs      # FLIR R-JPEG (APP1 chunks, PNG raw data)
├── image.rs          # ThermalImage, stats, CDF, camera info, recompute
├── planck.rs         # raw counts to Celsius, atmospheric transmission
├── exif.rs           # EXIF camera metadata
├── palette.rs        # palettes, mappings, overscan, units, rendering
├── doc.rs            # an open file: image, look, points, overlays
├── overlay.rs        # crosshairs, labels, min/max markers, corner pills
├── colorbar.rs       # the interactive colour bar
├── app.rs            # the egui app
├── export.rs         # PNG export: image plus tessellated overlays
├── raster.rs         # software rasteriser for egui meshes
├── headless.rs       # drives the app without a window, for tests
├── host.rs           # file open/save, native and web
└── main.rs           # GUI and command line
web/                  # WebAssembly entry, index.html, static files
```

### Data Flow

```
file bytes → formats::parse → ThermalImage (Celsius grid, min/max, CDF, metadata)
                                    ↓
                     Doc (look: palette, window, mapping; points; overlays)
                                    ↓
                Look::render → texture, overlay::paint on top, colorbar beside it
```

### Design Decisions

- **Temperature-driven rendering.** Pixels are coloured from their Celsius value, computed once at parse time. Palette, window and mapping changes never re-parse.
- **One colour function.** The image, the colour bar and the export all call `Look::rgba`, so the legend cannot disagree with the picture.
- **One overlay painter.** `overlay::paint` draws onto an egui `Painter` for the screen and into a headless context for export, so the PNG matches what you see.
- **Raw counts kept where they exist.** FLIR R-JPEG keeps its sensor counts and Planck constants, so calibration edits recompute every pixel.

---

## Roadmap

See [`plan.md`](plan.md) for the full market research and feature gap analysis against Fluke Connect, FLIR Thermal Studio, Testo IRSoft, and HIKMICRO Analyzer.

### Phase 1 — Table Stakes

- [x] **Editable parameters**: emissivity, reflected temp, air temp, humidity, distance, for FLIR R-JPEG
- [ ] **Measurement shapes** — box/rectangle, ellipse/circle, line (with min/max/avg), delta between two spots
- [ ] **Isotherms / color alarms** — highlight pixels above/below/between configurable thresholds
- [ ] **Text annotations** — free-text notes placed on the image
- [ ] **CSV export** — measurement data and/or full pixel grid export

### Phase 2 — Reports

- [ ] **PDF report generation** — pre-defined template with image, palette bar, measurements table, metadata
- [ ] **Multi-format file support** — radiometric JPEG minimum; `.is2` and `.seq` stretch goals
- [ ] **Side-by-side comparison** — load two images, synced level/span, visual diff

### Phase 3 — Power Tools

- [ ] **Batch processing** — apply palette, params, measurements to multiple files; bulk export
- [x] **Video frame extraction** — scrub and step through FLIR CSQ sequences on desktop
- [ ] **Custom report templates** — user-editable layout

---

## Why ThermView?

Commercial thermography software is Windows-only, expensive, and locked to specific camera brands. FLIR Thermal Studio Pro costs hundreds per year. Fluke Connect is tied to Fluke hardware. Testo IRSoft is PC-only.

ThermView runs in any browser and natively on any OS, and the IRG parser works on files from multiple Infiray camera variants — no vendor lock-in. The goal is to be the **VS Code of thermal imaging**: fast, cross-platform, extensible, and free.

---

## Contributing

Areas of particular interest:

- Additional IRG format variants (other Infiray camera models — send us a sample file)
- Radiometric JPEG parsing (`.jpg` with embedded temperature data)
- New color palettes
- Performance improvements for large images

Open an issue or PR.

---

## References

ThermView's format parsers are informed by prior art and community-maintained documentation:

- **[exiftool FLIR tags](https://exiftool.org/TagNames/FLIR.html)** — Canonical reference for FLIR AFF/FFF and R-JPEG binary structures, Planck calibration constants, and metadata field mappings.
- **[Thermimage R package](https://github.com/gtatters/Thermimage)** (gtatters) — Implements `raw2temp()` Planck-temperature conversion and `readflirJPG()` for FLIR JPEG extraction. The raw-to-Celsius formula in `src/planck.rs` is ported from Thermimage.
- **[FlirImageExtractor](https://github.com/Nervengift/read_thermal.py)** (Nervengift) — Python tool for unpacking FLIR JPEGs via exiftool and PIL. Established the byte-swap pattern for malformed FLIR 16-bit PNG data.
- **[thermal_parser](https://github.com/SanNianYiSi/thermal_parser)** (SanNianYiSi) — Multi-format Python parser supporting FLIR and DJI cameras. Demonstrated the chunked-APP1 reassembly approach (multiple `0xFFE1` segments per JPEG).
- **[infiray_irg](https://github.com/jaseg/infiray_irg)** (jaseg) — Python reference implementation of the Infiray IRG binary format, used to validate the IRG parser.
- **[Minkina & Dudzik — *Infrared Thermography: Errors and Uncertainties*](https://www.wiley.com/en-us/Infrared+Thermography%3A+Errors+and+Uncertainties-p-9780470747186)** — Foundational reference for the atmospheric transmission and Planck radiometry formulas.

## License

GPL-3.0-or-later. Copyright © 2026 Kieran Harkin. You may share and change it under the terms of
the GNU General Public License, version 3 or any later version; see [LICENSE](LICENSE). Releases
before the next release were published under MIT and stay available under it.
