# File-specific temperature conversion

Date: 15 November 2026. These changes apply to the current source, after the
[0.1.11 audit](temperature-validation.md). The original audit and its results
remain a record of the released 0.1.11 behavior.

FLIR CSQ and radiometric JPEG conversion now reads atmospheric alpha 1/2,
beta 1/2 and X from CameraInfo offsets 0x70–0x80, plus IR-window temperature
and transmission at 0x30/0x34. The fields are retained with each frame's raw
counts and Planck calibration, and reused during recalibration. The displayed
atmospheric transmission uses those same coefficients.

Stored zero distance and humidity remain zero. Humidity follows ExifTool's
convention: stored values above 2 are percentages; other values are multiplied
by 100. The normalized value must be within 0–100%. The distance control accepts
zero and does not clamp existing file distances to its editing range.

CameraInfo reads are bounded to the declared record. Missing, nonfinite or
invalid required calibration/environmental fields produce a loading error;
the CSQ/R-JPEG parser no longer substitutes generic Planck constants or searches
unrelated bytes for plausible calibration. Zero Planck O is preserved. Legacy
AFF conversion retains its existing handling; this change does not independently
validate other camera formats.

## Comparison with the saved references

The 20 original extracted CSQ source frames were concatenated in their original
order into a smaller CSQ and read through the updated video/parser pipeline.
The three original JPEG inputs were also reopened. Updated pixel arrays were
compared against the saved, unmodified Thermimage file-metadata reference arrays
from the original audit, with the same 0.001°C threshold and exact raw-count
comparison. Original frame numbers are retained in the result labels.

- All **22 readable cases pass**, covering **1,920,000 pixels** including the
  duplicate exported JPEG content.
- **Zero raw-count mismatches** and no validity-mask differences in readable cases.
- Largest absolute temperature difference: **0.00000190724°C** (rounded upward).
- CSQ frame **5527 still fails JPEG-LS decoding**; no temperature is validated
  for it. This remains incomplete coverage, not a successful full-recording test.

[Per-case results](validation/temperature-file-coefficients-2026-11-15.json)
record each comparison. Raw replay outputs are in the ignored local directory
`target/temperature-validation-file-coefficients/`. The replay script used for
this local check is `/tmp/thermview-replay-file-coefficients.py`.

Regression tests also cover nonzero distance, nondefault atmospheric coefficients,
an absorbing IR window, both CameraInfo byte orders, recalibration, zero values,
percentage humidity, and rejection of invalid or truncated metadata. Synthetic
CSQ fixtures now explicitly supply their environmental fields.

These checks establish software agreement for the tested inputs. They do not
establish that stored experimental settings are physically correct, provide a
vendor-software comparison, or establish uncertainty against a calibrated
physical temperature reference.
