# Temperature validation: ThermView 0.1.11

Date: 13 November 2026. This is an audit of the existing temperature conversion;
the conversion code was not changed to make the comparisons pass.

## Findings

ThermView reads thermal pixel values and calibration metadata. It does not infer
temperatures from the visible colours. This was checked both in the source and
by replacing a radiometric JPEG's visible image with solid black and magenta:
every raw count and every computed temperature remained bit-for-bit identical.

The numerical results are mixed:

| Data | Coverage | Result against file-metadata references |
|---|---|---|
| Local FLIR CSQ recording | 20 selected frames out of 16,702; 19 readable | Every readable raw sensor count matched. Largest temperature difference: **0.16928354°C**. |
| Two radiometric JPEGs exported from that recording | Complete 320 × 240 rasters | Every raw count matched. Largest temperature differences: **0.10281155°C** and **0.10285106°C**. These are container checks, not independent camera samples. |
| Thermimage `IR_2412.jpg` example | Complete 640 × 480 raster | Every raw count matched. Largest temperature difference: **0.00000191°C**. |
| CSQ frame 5527 | Deliberately included because it previously failed | ThermView rejected the JPEG-LS stream. FFmpeg produced a raster, but its reference temperature conversion contained **39 invalid pixels**. No validated ThermView temperature is reported for this frame. |

Across the 22 readable cases, **1,920,000 pixel comparisons** were made, with
**zero raw-count mismatches**. Duplicate content in exported JPEGs is included
in this count; it is not 1,920,000 independent physical measurements.

The two external temperature converters agreed within **5 × 10⁻¹⁴°C** on finite
reference values. With those converters explicitly given ThermView's current
assumptions instead of the file's settings, ThermView agreed within
**0.00000191°C**. That diagnostic isolates the observed discrepancy to settings
and assumptions rather than numerical precision or raw-image decoding. It is
reported separately and does not turn the file-metadata comparisons into passes.

The acceptance threshold was **0.001°C per pixel**, with matching validity masks
and exact raw counts. This deliberately tests software agreement; it is not a
camera accuracy specification.

## The discrepancy in the local recording

In `07_10_26_14_30_Back_Circle_Thermal.csq`, ExifTool reads:

| Quantity | File metadata | ThermView 0.1.11 |
|---|---|---|
| Emissivity | 0.9599999785 | Same |
| Object distance | **0 m** | **1 m**, substituted when the stored value is zero |
| Reflected apparent temperature | 16.99999390°C | Same |
| Atmospheric temperature | 16.99999390°C | Same |
| Relative humidity | 0.5699999928 fraction = 56.99999928% | Same after unit conversion |
| IR-window transmission | 1 | Fixed at 1 |
| Atmospheric alpha 1 / alpha 2 | 1.239000014e-8 / 1.109499959e-8 | Fixed at 0.006569 / 0.01262 |
| Atmospheric beta 1 / beta 2 | 0.003180000000 / 0.003180200001 | Fixed at −0.002276 / −0.00667 |
| Atmospheric X | 0.7319999933 | Fixed at 1.9 |

All five Planck calibration constants matched ExifTool in every readable case:
for this recording, R1 = 16002.7138671875, B = 1417.800048828125, F = 1,
O = −5030, R2 = 0.011601288802921772.

At zero distance the reference atmospheric transmission is 1, independently of
the atmospheric coefficients. ThermView's substitution of 1 metre activates its
fixed atmospheric correction. Giving the reference converter the substituted
distance and fixed coefficients reproduces ThermView's result to float precision.

For example:

| Case | ThermView min / max (°C) | File-metadata reference min / max (°C) |
|---|---|---|
| CSQ frame 1 | 11.098119 / 30.050652 | 11.149079 / 29.947840 |
| CSQ frame 4555 | 11.204765 / 39.300415 | 11.254775 / 39.131131 |
| CSQ frame 11148 | 11.667571 / 30.502344 | 11.713471 / 30.396179 |
| CSQ frame 16702 | 12.345128 / 32.921303 | 12.385050 / 32.797372 |
| `IR_2412.jpg` | 22.735895 / 35.250450 | 22.735894 / 35.250450 |

Agreement with stored metadata does not establish that the stored settings are
physically correct. Zero distance can be a recording placeholder. A physical
validation also requires the actual distance, emissivity, reflected temperature,
environment and camera calibration to be appropriate for the experiment.

## How CSQ and FLIR radiometric JPEGs become temperatures

1. **Find the thermal container.** A CSQ contains indexed FFF frames; each frame's
   directory identifies RawData (record type 1) and CameraInfo (type 32). A FLIR
   radiometric JPEG carries the FFF structure in FLIR APP1 metadata chunks,
   alongside its separate visible JPEG image.
2. **Decode thermal values.** ThermView obtains one 16-bit sensor count per thermal
   pixel from the raw record: uncompressed values, a thermal PNG, or JPEG-LS.
   The CSQ JPEG-LS decoder is CharLS. These values are not RGB brightnesses.
3. **Read calibration and environmental settings.** The CameraInfo record supplies
   the five Planck coefficients and environmental fields below. The current
   defaults and omissions are described explicitly later in this report.
4. **Correct for the environment and invert the calibration equation.**
   `src/planck.rs` calculates atmospheric transmission, removes reflected and
   atmospheric contributions in sensor-count units, and converts the corrected
   object signal into Celsius.
5. **Measure the temperature array.** Minimum, maximum and point readings come
   from that numeric array. Video frames are decoded separately; the normal
   unmodified view uses each frame's metadata. User calibration adjustments are
   recomputed from retained raw counts and applied across the video until reset.
6. **Apply display colours.** The selected palette, range and mapping transform
   the temperatures into display pixels afterward. They do not set the measured
   temperatures. Celsius/Fahrenheit display conversion is also separate.

CameraInfo offsets used by the CSQ/FLIR JPEG parser are relative to the beginning
of that record, not the whole file:

| Offset | Field | Interpretation |
|---|---|---|
| `0x20` | Emissivity | Dimensionless float |
| `0x24` | Object distance | Metres |
| `0x28` | Reflected apparent temperature | Kelvin, converted to Celsius |
| `0x2c` | Atmospheric temperature | Kelvin, converted to Celsius |
| `0x3c` | Relative humidity | Fraction converted to percent by the current parser |
| `0x58`, `0x5c`, `0x60` | Planck R1, B, F | Camera calibration floats |
| `0x308`, `0x30c` | Planck O, R2 | Signed integer offset and float multiplier |

These offsets agree with [ExifTool's FLIR CameraInfo definitions](https://github.com/exiftool/exiftool/blob/1da714924f67763b6a8a7013dd2aa1983a16f87b/lib/Image/ExifTool/FLIR.pm).
CameraInfo byte order is detected by the parser. Other FLIR AFF variants use a
different layout and are handled by a separate parser.

The final inverse calibration equation is:

```text
T_C = B / ln(R1 / (R2 × (S_object + O)) + F) − 273.15
```

`S_object` is the raw sensor signal after emissivity, reflected radiation and
atmospheric corrections. It is not a colour value. The environmental correction
used by ThermView follows the same equation family as
[Thermimage's raw2temp](https://github.com/gtatters/Thermimage/blob/cb244dac8fffaac50b1ba8bbbf58b7d37b5d96e1/R/raw2temp.R).

Relevant implementation files:

- [`src/video.rs`](../src/video.rs): CSQ indexing and per-frame loading.
- [`src/formats/rjpeg.rs`](../src/formats/rjpeg.rs): FFF/APP1 extraction, metadata and raw decoding.
- [`src/planck.rs`](../src/planck.rs): environmental correction and inverse calibration.
- [`src/image.rs`](../src/image.rs): numeric arrays, extrema and recomputation.
- [`src/palette.rs`](../src/palette.rs): colours generated from temperatures.
- [`src/doc.rs`](../src/doc.rs) and [`src/measure.rs`](../src/measure.rs): scrubbing, calibration overrides and range analysis.

## Other image formats

Not every supported format stores unconverted sensor counts. Some store a
temperature plane already converted by the camera:

| Format | Current interpretation | External validation in this audit |
|---|---|---|
| FLIR R-JPEG and CSQ | Raw counts plus CameraInfo, converted using Planck coefficients | Sampled as described above |
| Infiray IRG | Fine 16-bit thermal plane: value / 16 or value / 10 Kelvin, depending on variant; subtract 273.15 | Not tested against vendor software |
| Hikmicro HDRI JPEG | Thermal values / 100 Celsius from the HDRI block | Not tested against vendor software |
| Supported DJI JPEG variants | Embedded APP3 thermal values / 64 Kelvin; subtract 273.15 | Not tested against DJI software |
| Older FLIR AFF IMG/SEQ | Either centikelvin values or raw-count conversion, selected by an existing sample-value heuristic | Not independently validated here |

For formats with finished temperature planes, displayed environmental metadata
is not used to independently recalculate those temperatures. Ordinary colour
JPEGs without recognized thermal data are rejected.

## Limitations and changes indicated by the audit

- **Stored zero values can be replaced.** CSQ/FLIR JPEG distance 0 becomes 1 m;
  zero humidity becomes 50%. Zero distance and zero humidity are meaningful
  inputs to a reference calculation and should not silently imply different
  settings. Invalid/missing metadata needs separate handling.
- **Atmospheric coefficients are fixed.** The current converter does not read
  CameraInfo atmospheric alpha, beta or X values, even when present.
- **The IR window is assumed transparent.** Transmission is fixed at 1 and
  window temperature at atmospheric temperature. Files describing an absorbing
  window may differ substantially from software that uses those fields.
- **Calibration fallback is not strict.** Missing, nonfinite or zero Planck
  values can be replaced by generic constants. The Planck O/R2 fallback can
  search for plausible byte patterns. Such results should not be called validated
  camera-specific measurements without confirming the actual metadata.
- **Humidity representation varies.** ExifTool recognizes both fractional and
  percentage storage; ThermView's current FLIR parser only accepts the fractional
  range and otherwise substitutes 50%. The tested files used fractional storage.
- **Corrupt frames remain failures.** Matching temperatures on healthy frames
  does not validate damaged frames. Range analysis skips rejected frames and
  reports them; its statistics cover successful frames only. The independent
  decoder accepting frame 5527 does not make its damaged raster trustworthy.
- **Sampling is not exhaustive validation.** This run tested 20 CSQ frames, not
  all 16,702. The JPEG-LS padding case at frame 11148 did match the independent
  decoder exactly. Other cameras, temperature ranges and file layouts need tests.
- **The references share ancestry.** Thermimage and AlexanderProd/csq implement
  the same published conversion family. Their agreement is useful verification
  of implementation, not two independent physical models.
- **No vendor or blackbody comparison has been performed.** This audit did not
  run FLIR Research Studio/Tools, the FLIR SDK, DJI software or a calibrated
  temperature source. Software agreement alone cannot establish absolute accuracy.

Recommended next changes are to preserve legitimate zero settings, use the
file's atmospheric and IR-window fields, and expose missing calibration instead
of silently supplying it. Those changes should be validated against these same
independent references before being released. This audit does not alter 0.1.11.

## Reference methods and reproducibility

Audited application version: **0.1.11**, released source commit
`6881f3d2ff73dfa19eb8598ca63f55f6466b1a1d`. The checkout at execution was
`b99568af495ef489eb11ca2b735bf96f82e9961d`; its subsequent changes were workflow
action versions, with no differences in `src/`, `Cargo.toml` or `Cargo.lock`.
The installed `/usr/bin/thermview` also reported 0.1.11 and the same rounded
extrema for the first exported JPEG.

The reference pipeline used:

- **ExifTool 13.63**, commit `1da714924f67763b6a8a7013dd2aa1983a16f87b`, for
  independent metadata and embedded thermal payload extraction.
- **FFmpeg 6.1.1**, JPEG-LS decoder, for CSQ thermal decoding; this is a different
  decoder from ThermView's CharLS. Pillow decoded embedded TIFF/PNG rasters.
- **Thermimage**, commit `cb244dac8fffaac50b1ba8bbbf58b7d37b5d96e1`, running
  its unmodified `raw2temp.R` in **R 4.6.2**. This uses the upstream conversion
  function, not a separately launched graphical application.
- **AlexanderProd/csq**, commit `a0ea1a043a312e7869d23ce0df586ec480eef3e4`,
  executing its unmodified `raw2temp` function in Python/NumPy. Its file reader
  and optional dependencies were not used. Input humidity was explicitly
  normalized to percent for both reference converters.

The references used all available file environmental/calibration fields, with
no generic calibration substitution. Separately recorded diagnostics used
ThermView's assumptions; these are labelled and excluded from acceptance.
Raw samples, full temperature rasters and extracted metadata remain in the local
ignored `target/temperature-validation/` directory. Camera imagery, serial
numbers and GPS fields are not included in the summary artifacts below.

Results:

- [Per-case CSV](validation/temperature-comparison-2026-11-13.csv)
- [Detailed JSON, including hashes, metadata, diagnostics and controls](validation/temperature-comparison-2026-11-13.json)

The selected CSQ frames (one-based, matching the interface) were:
1, 2, 3, 1519, 3037, 4555, 5526, 5527, 5528, 6074, 7592, 9110, 10628,
11147, 11148, 11149, 12147, 13665, 15183, 16702.

To repeat a comparison, use Python 3.12+ with NumPy and Pillow, plus Perl,
FFmpeg, Rscript and the project's Rust toolchain. Fetching references downloads
public source files and verifies pinned SHA-256 hashes; it does not install
system packages or upload local recordings:

```bash
python3 tools/fetch_temperature_references.py
cargo build --locked --example temperature_dump

python3 tools/validate_temperatures.py \
  --csq /path/to/recording.csq \
  --image target/temperature-reference-tools/IR_2412.jpg \
  --tools target/temperature-reference-tools \
  --exiftool "$(cat target/temperature-reference-tools/exiftool-root.txt)/exiftool"
```

For a smaller check add `--frames 1,2,3`. Add further `--image` arguments for
FLIR radiometric JPEGs. The CSV/JSON comparison report above contains the exact
recording and individual source-frame hashes. No local camera data is needed to
repeat the public JPEG comparison: omit `--csq`.

The validator exits **1** when comparisons differ, references fail, or ThermView
rejects a selected frame; that is expected for this documented 0.1.11 audit.
It exits **0** only when every selected case passes. It writes partial results
as each case finishes. The complete run log from this audit is locally available
at `/tmp/thermview-temperature-comparison.log`, with its exit status in
`/tmp/thermview-temperature-comparison.exit`.

For the remaining vendor check, export a full-frame temperature CSV from FLIR
software using precisely the same frame, resolution, environmental settings,
window settings, temperature units and pixel coordinates. Record the software
version and whether it uses embedded or overridden parameters. Compare all
pixels and validity masks before comparing min/max. Finally, validate physical
accuracy with a traceable temperature reference under the experiment's actual
measurement conditions.
