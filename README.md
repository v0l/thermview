# ThermView

**A web-based thermal imaging analysis tool — built for thermographers, not just developers.**

ThermView parses, renders, and analyzes radiometric thermal images in the browser. It supports Infiray `.irg` binary files, Hikmicro JPEGs (Pocket2, SP60), FLIR R-JPEG files (E40, T640, AX8, B60, etc.), and DJI R-JPEG files (Mavic 2 Enterprise Advanced). Features real-time palette switching, a canvas-based draggable temperature range bar, CDF histogram equalization, spot measurements with on-canvas min/max markers, and PNG export.

<p align="center">
  <img src="public/favicon.svg" width="64" alt="ThermView icon" />
</p>

<p align="center">
  <a href="#features">Features</a> ·
  <a href="#getting-started">Getting Started</a> ·
  <a href="#usage">Usage</a> ·
  <a href="#file-formats">Formats</a> ·
  <a href="#architecture">Architecture</a> ·
  <a href="#roadmap">Roadmap</a>
</p>

---

## Features

### Current

| Feature | Description |
|---|---|
| **Multi-format parsing** | Auto-detects IRG, Hikmicro JPEG (Pocket2/SP60), FLIR R-JPEG, and DJI R-JPEG (M2EA). Format-agnostic `ThermalImage` abstraction — add a parser, not rewrite the viewer. |
| **9 color palettes** | Inferno, Iron, Jet, Hot, Lava, Plasma, Rainbow, Arctic, Grayscale |
| **Canvas range bar** | Single-canvas render with panning gradient strip, dimmed out-of-range regions, tick marks, and draggable handle values |
| **Scale modes** | Linear, logarithmic, and **histogram equalization** (1024-bin CDF LUT, built at parse-time) |
| **Overscan** | clip, hide, below-color, above-color |
| **Min/max spot overlay** | ▲ red-hot max and ▼ blue-cold min triangle markers with live temperature labels |
| **Info overlays** | Emissivity pill and file timestamp pill on-canvas corners |
| **Spot measurements** | Click-to-place, crosshair with label, rename via sidebar panel |
| **Hover readout** | Real-time pixel temperature as you move the mouse |
| **Unit conversion** | Toggle between Celsius (°C) and Fahrenheit (°F) |
| **Palette inversion** | Flip hot/cold colors |
| **Image scaling** | 1×–6× nearest-neighbor upscale |
| **PNG export** | Download the rendered thermal image |
| **Metadata display** | Emissivity, air temp, ref temp, distance, atmospheric transmittance |
| **Dark theme** | Purpose-built thermal imaging UI with noise-texture background |
| **Drag & drop** | Drop thermal files (`.irg`, `.jpg`, `.jpeg`, `.img`) directly onto the app |

---

## Getting Started

### Prerequisites

- [Bun](https://bun.sh/) ≥ 1.x

### Install

```bash
git clone https://github.com/user/thermview.git
cd thermview
bun install
```

### Development

```bash
bun run dev        # Start Vite dev server at http://localhost:5173
```

### Production

```bash
bun run build      # TypeScript check + Vite production build
bun run preview    # Preview the production build locally
```

---

## Usage

1. Open `http://localhost:5173`
2. Drop a `.irg` file onto the window (or click the drop zone to browse)
3. Select a color palette from the toolbar
4. Drag the range bar handles to adjust the temperature window
5. Click the image to place measurement points
6. Toggle °C/°F, scale mode (linear / log / equalize), overscan, inversion as needed
7. Toggle overlay layers (min/max spots, info pills, timestamp)
8. Export the view as PNG

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

**File structure:** JPEG → APP1 chunk(s) with `FLIR\0` header → FFF record directory → RawData PNG + CameraInfo (Planck constants, emissivity, etc.). Large APP1 payloads may be split across multiple chunks.

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

Where `tempScale` is 16 for C210 cameras (`flag0 = 0`) or 10 for all other variants. The parser computes the full Celsius grid (`Float32Array`) and min/max/CDF at parse time — consuming code never touches raw values.

---

## Architecture

```
src/
├── components/
│   ├── ThermalViewer.tsx     # Main viewer — file I/O, toolbar, layout, state
│   ├── ThermalCanvas.tsx     # Canvas renderer — image, crosshairs, labels, overlays
│   ├── RangeColorBar.tsx     # Canvas-based interactive level/span with gradient, ticks, handles
│   ├── CursorPanel.tsx       # Measurement point list with rename/delete
│   └── ui/                   # shadcn/ui primitives (button, card, select, slider, label)
├── lib/
│   ├── irg-parser.ts         # Binary parser + palette system + temp→canvas rendering
│   ├── types.ts              # Shared TypeScript types (ThermalImage, RenderOpts, etc.)
│   ├── constants.ts          # Palette definitions, cursor colors, overscan options
│   ├── units.ts              # °C ↔ °F conversion
│   └── drawing.ts            # Canvas crosshair, label, marker, and pill drawing primitives
├── App.tsx                   # Root component
├── main.tsx                  # React DOM entry
└── index.css                 # Tailwind v4 theme + global styles
```

### Data Flow

```
.irg file → parseIRG(buf) → ThermalImage (celsius grid, min/max, CDF LUT, metadata)
                                    ↓
                          ThermalViewer (state: range, palette, scale mode)
                                    ↓
                          createTempCanvas(celsius, w, h, renderOpts) → offscreen canvas
                                    ↓
                          ThermalCanvas: blit scaled + draw cursors/overlays on top
```

### Tech Stack

| Layer | Choice |
|---|---|
| Framework | React 19 |
| Language | TypeScript |
| Bundler | Vite 8 |
| Runtime | Bun |
| UI primitives | shadcn/ui + Base UI |
| Styling | Tailwind CSS v4 |
| Fonts | JetBrains Mono (display), Inter Variable (body) |

### Design Decisions

- **No runtime dependencies for image processing.** All thermal rendering uses the Canvas 2D API directly — no WebGL, no image processing libraries.
- **Gradient-stop palette system.** Each palette is defined as an array of `{pos, r, g, b}` stops with linear interpolation. Easy to add or customize palettes.
- **Temperature-driven rendering.** Pixels are colored by their actual Celsius temperature (a `Float32Array` computed once at parse time), not their 8-bit luminance value. Palette, range, and scale mode changes are all correct without re-parsing.
- **ThermalImage abstraction.** The parser produces a decoded image structure with all computations done upfront (Celsius conversion, min/max spots, CDF LUT). Adding a new file format means writing one parser function that returns a `ThermalImage`.
- **Offscreen canvas for source data.** The base thermal image is rendered once into an offscreen canvas, then drawn scaled to the display canvas. Cursors and labels are drawn on top for each frame.
- **Canvas-based range bar.** All elements (gradient, ticks, labels, dimmed overlays, handle dots, handle lines, readout values) are rendered in a single `<canvas>` — no HTML-overlay z-index jank.

---

## Roadmap

See [`plan.md`](plan.md) for the full market research and feature gap analysis against Fluke Connect, FLIR Thermal Studio, Testo IRSoft, and HIKMICRO Analyzer.

### Phase 1 — Table Stakes

- [ ] **Editable parameters** — emissivity, reflected temp, air temp, humidity, distance (re-renders on change)
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

ThermView runs in any browser, on any OS, and the IRG parser works on files from multiple Infiray camera variants — no vendor lock-in. The goal is to be the **VS Code of thermal imaging**: fast, cross-platform, extensible, and free.

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
- **[Thermimage R package](https://github.com/gtatters/Thermimage)** (gtatters) — Implements `raw2temp()` Planck-temperature conversion and `readflirJPG()` for FLIR JPEG extraction. The raw-to-Celsius formula used in `flir-parser.ts` and `flir-rjpeg-parser.ts` is ported from Thermimage.
- **[FlirImageExtractor](https://github.com/Nervengift/read_thermal.py)** (Nervengift) — Python tool for unpacking FLIR JPEGs via exiftool and PIL. Established the byte-swap pattern for malformed FLIR 16-bit PNG data.
- **[thermal_parser](https://github.com/SanNianYiSi/thermal_parser)** (SanNianYiSi) — Multi-format Python parser supporting FLIR and DJI cameras. Demonstrated the chunked-APP1 reassembly approach (multiple `0xFFE1` segments per JPEG).
- **[infiray_irg](https://github.com/jaseg/infiray_irg)** (jaseg) — Python reference implementation of the Infiray IRG binary format, used to validate the IRG parser.
- **[Minkina & Dudzik — *Infrared Thermography: Errors and Uncertainties*](https://www.wiley.com/en-us/Infrared+Thermography%3A+Errors+and+Uncertainties-p-9780470747186)** — Foundational reference for the atmospheric transmission and Planck radiometry formulas.

## License

MIT
