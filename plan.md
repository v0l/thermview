# ThermView — Market Research & Feature Gap Analysis

## Sources

- **Fluke** — [Inspection Software blog](https://www.fluke.com/en-us/learn/blog/thermal-imaging/inspection-software), Fluke Connect Desktop, SmartView Classic, SmartView R&D
- **FLIR** — [Thermal Studio Suite](https://www.flir.com/en-eu/products/flir-thermal-studio-suite/) (Starter / Standard / Professional tiers), [version comparison matrix](https://flir.custhelp.com/app/answers/detail/a_id/4590), datasheet
- **Testo** — IRSoft v5.2 (licence-free PC software)
- **HIKMICRO** — HIKMICRO Analyzer/Viewer
- **InfraTec** — IRBIS®3
- **jaseg/infiray_irg** — Python reference implementation of the Infiray IRG binary format (used to validate parsing)

---

## Industry Standard Features

Every professional thermography software package includes:

1. **Multi-format file support** — proprietary radiometric formats (`.is2`, `.seq`, `.csq`, `.bmt`) + standard image/video formats (JPG, MP4, BMP, TIFF, GIF, AVI)
2. **Edit & manipulate images** — adjust level/span, emissivity, reflected temperature, background temperature, humidity, distance, atmospheric transmission
3. **Visible light + IR blending** — thermal fusion, picture-in-picture, MSX alignment (FLIR), IR-Fusion (Fluke)
4. **Live camera streaming** — view and optionally record radiometric video from the camera in real time
5. **Remote camera control** — trigger auto-focus, capture, and settings without touching the camera
6. **Report generation** — templated or custom reports exported as PDF, DOCX, XPS, HTML
7. **3D analysis** — view IR images from different perspectives (Fluke 3D-IR)
8. **Side-by-side comparison** — compare baseline, historical, and current images of the same asset
9. **Color palette control** — standard palettes + custom palette creation
10. **Annotations** — text, voice/audio notes, visible-light reference images attached to IR images
11. **Asset catalog & tagging** — categorize, tag, and associate images with equipment in a hierarchy
12. **Measurement tools** — spot, box, ellipse, line, polygon, polyline, magic wand, delta temperature, profile plots, isotherms/color alarms
13. **Batch processing** — apply palette, params, measurements, and export across hundreds of images
14. **CSV/XLS temperature export** — export pixel-level or measurement temperature data for external analysis
15. **Route-based inspection** — predefined walkthrough paths downloaded to camera, organizing data by asset

---

## ThermView Current State

### Supported Formats

| Format | Cameras | Resolution | Encoding |
|---|---|---|---|
| IRG (binary) | Infiray C200/C210, Autel Evo II Dual, P200 | varies | int16 LE / scale = °K |
| Hikmicro JPEG | Pocket2, SP60 | 256×192 / 640×480 | uint16 LE centi-°C |
| DJI R-JPEG | Mavic 2 Enterprise Advanced | 640×512 | uint16 LE / 64 = K |

### Feature Status

| Feature | Status |
|---|---|
| IRG file parsing | ✅ Multi-variant (C200/C210 0xACCA, Autel, P200 0x04A0, unknown 0x0BB0/0xBAAB with CAAC tail) |
| Format-agnostic image model | ✅ `ThermalImage` abstraction — parsers produce, viewers consume |
| Color palettes (9) | ✅ Strong gradient-stop system |
| Level/span (draggable range bar) | ✅ Single-canvas render with panning gradient strip |
| Spot measurement points | ✅ Click-to-place, draggable rename |
| Unit conversion (°C/°F) | ✅ |
| Hover temperature readout | ✅ |
| Scale modes | ✅ Linear, logarithmic, **histogram equalization** |
| Overscan modes | ✅ clip / hide / below / above |
| Min/max spot overlay | ✅ Triangle markers (▲ max, ▼ min) with temperature labels |
| Info overlays | ✅ Emissivity pill + file timestamp pill |
| PNG export | ✅ |
| Temperature metadata display | ✅ Read-only (emissivity, air/ref temp, distance, atm trans) |

---

## Feature Gap: Tier 1 — Table Stakes

Every competitor has these. ThermView has none of the critical ones yet.

| # | Feature | Priority | Status | Notes |
|---|---|---|---|---|
| 1 | **Report generation (PDF)** | Critical | ❌ | Predefined + custom templates; the primary deliverable in thermography |
| 2 | **Parameter editing** | Critical | ❌ | Emissivity, reflected temp, air temp, humidity, distance, atm trans — all must be editable and must re-render the image |
| 3 | **Multi-shape measurements** | Critical | ❌ | Box/rectangle, ellipse/circle, line, polygon, polyline — not just single-point spots |
| 4 | **Delta temperature** | Critical | ❌ | Difference between two measurement points |
| 5 | **Isotherms / color alarms** | High | ❌ | Highlight all pixels above/below/between thresholds |
| 6 | **CSV temperature export** | High | ❌ | Export measurement data for external analysis |
| 7 | **Image annotations** | High | ❌ | Text labels, notes attached to the image or report |
| 8 | **Image rotation & crop** | Medium | ❌ | 90° rotation, free rotation, crop to region of interest |
| 9 | **Multi-format file support** | — | ✅ DONE | IRG, Hikmicro (Pocket2/SP60), DJI (M2EA) |
| 10 | **Min/max auto-spot** | — | ✅ DONE | Triangle markers rendered on-canvas with live °C labels |
| 11 | **Histogram equalization** | — | ✅ DONE | 1024-bin CDF LUT built at parse-time, consumed by `equalize` scale mode |

---

## Feature Gap: Tier 2 — Professional Differentiation

What separates "pro" from "starter" tiers in commercial software.

| # | Feature | Priority | Status | Notes |
|---|---|---|---|---|
| 12 | **Side-by-side comparison** | High | ❌ | Load two images, sync level/span, visually compare asset condition over time |
| 13 | **Batch processing** | High | ❌ | Apply palette, params, and measurements to a folder of images; export all at once |
| 14 | **Video support** | Medium | ❌ | Playback and frame extraction from `.seq`, `.csq`, `.mp4` radiometric video |
| 15 | **Live camera streaming** | Medium | ❌ | USB/RTSP stream from supported cameras |
| 16 | **Visible light + IR blending** | Medium | ❌ | Overlay/blend visible-light image with thermal if camera provides both |
| 17 | **Panorama stitching** | Low | ❌ | Combine multiple images into a wide-FOV composite |
| 18 | **Route-based inspection** | Low | ❌ | Guided inspection workflows downloaded to camera |
| 19 | **3D view** | Low | ❌ | Perspective rendering of IR data (Fluke 3D-IR) |

---

## Architecture Observations

Strengths to build on:

- **Rendering pipeline is solid** — `createTempCanvas`, palette system with gradient stops, scale modes, overscan, all well-structured
- **UI is polished** — dark theme, canvas-based range bar, cursor panel, stat pills are professional
- **TypeScript throughout** — clean type definitions, well-factored into separate modules
- **`ThermalImage` abstraction** — binary parsers produce a common decoded-IR structure; viewers never touch raw bytes. Adding a new file format means writing one parser function.

Key architectural changes still needed:

1. **State management** — Currently all state lives in `ThermalViewer`. Adding measurements, annotations, parameter editing, reports, and batch processing will require a store (Zustand or Jotai) or at minimum a context + reducer pattern.
2. **Undo/redo** — Parameter edits, measurement placement, and annotation changes need undo support.
3. **Report engine** — Template system → PDF generation (jsPDF or similar).
4. **Canvas interaction layer** — Currently click-to-add-spot. Need a tool mode system (spot / box / line / polygon / pan / zoom) with drag handles for resize/move of existing shapes.
5. **Image comparison view** — Side-by-side or overlay with synchronized level/span.

---

## Recommended Implementation Order

### Phase 1 — Table Stakes (v0.2)
1. Editable parameters (emissivity, reflected temp, air temp, humidity, distance)
2. Measurement shapes (box, ellipse, line + delta)
3. Isotherms / color alarms
4. Text annotations
5. CSV export of measurement data

### Phase 2 — Reports (v0.3)
6. PDF report generation with basic template
7. Multi-format file support (radiometric JPG minimum)
8. Side-by-side image comparison

### Phase 3 — Power Tools (v0.4)
9. Batch processing
10. Video frame extraction
11. Custom report templates
