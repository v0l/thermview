# ThermView

**A thermal imaging analysis tool for thermographers, not just developers.**

ThermView reads, renders and measures radiometric thermal images, natively on the desktop or in the browser through WebAssembly. It supports Infiray `.irg` files, Hikmicro JPEGs (Pocket2, SP60), FLIR R-JPEG files (E40, T640, AX8, B60 and others), FLIR AFF `.img` files and DJI R-JPEG files (Mavic 2 Enterprise Advanced). It is written in Rust with [egui](https://github.com/emilk/egui) and the [egui_bench](https://github.com/v0l/egui_bench) panel theme.

Part of the [bench suite](https://github.com/v0l/bench-suite), a launcher that installs, updates
and starts it alongside the other bench tools.

<p align="center">
  <img src="web/public/favicon.svg" width="64" alt="ThermView icon" />
</p>

---

## Features

| Feature | Description |
|---|---|
| **Multi-format parsing** | Detects IRG, Hikmicro JPEG, FLIR R-JPEG, FLIR AFF and DJI R-JPEG from the bytes. Every parser returns the same `ThermalImage`, so a new format is one parser. |
| **9 colour palettes** | Inferno, Iron, Jet, Hot, Lava, Plasma, Rainbow, Arctic, Grayscale, with inversion |
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
| **PNG export** | The scaled image with every marker and label on it, from the app or the command line |
| **Metadata** | EXIF make, model, lens, software, serial, capture time and GPS |
| **Drag and drop** | Drop a file onto the window, native or web |

## Getting Started

Needs Rust 1.92 or newer.

```bash
git clone https://github.com/v0l/thermview.git
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

1. Open a file with **open** (ctrl+O) or drop one onto the window
2. Pick a palette on the left
3. Drag the colour bar handles, type the window, or use a preset
4. Click the image to place measurement points, rename them on the right
5. Switch °C/°F, mapping, overscan, inversion, scale and overlays
6. For FLIR R-JPEG, edit the calibration on the right
7. **export** (ctrl+E) saves the view as a PNG next to the source file, or downloads it on the web

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
- [ ] **Video frame extraction** — step through radiometric video sequences
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
