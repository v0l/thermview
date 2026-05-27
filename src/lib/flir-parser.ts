import type { ThermalImage } from '@/lib/types';

/**
 * Parse a FLIR AFF (AGEMA File Format) `.IMG` thermal image into ThermalImage.
 *
 * Supports:
 *   - FLIR ThermaCAM PM695 (SC2000-style record layout)
 *   - Other FLIR cameras using the AFF/FFF binary format
 *
 * The AFF format consists of:
 *   1. File header (0x40 bytes) — "AFF\0" or "FFF\0"
 *   2. Record directory — entries pointing to binary data records
 *   3. Record type 1 ("AFF1") — raw thermal frame:
 *      a. Frame header: byte order, sensor dimensions, crop region
 *      b. Calibration data: emissivity, object distance, temperatures,
 *         Planck constants (R1, B, F, O, R2), atmospheric constants
 *      c. Raw pixel data: uint16 LE values (centi-Kelvin for classic
 *         cameras like PM695; or raw sensor AD counts for newer ones)
 *
 * Conversion from raw to Celsius depends on the camera model and the
 * interpretation of the raw values.
 *
 * References:
 *   - exiftool.org TagNames/FLIR.html  (AFF1/AFF5 tables)
 *   - exiftool forum topic 4898 (AFF format documentation)
 *   - Thermimage R package raw2temp.R
 *   - FlirImageExtractor Python library
 */
export function parseFLIR(buffer: ArrayBuffer): ThermalImage {
  const view = new DataView(buffer);
  const len = buffer.byteLength;

  // ── Validate AFF header ───────────────────────────────────────────────
  const magic = String.fromCharCode(
    view.getUint8(0), view.getUint8(1), view.getUint8(2)
  );
  if (magic !== 'AFF' && magic !== 'FFF') {
    throw new Error(`Not a FLIR AFF/FFF file (magic: "${magic}")`);
  }

  // ── Parse file header (big-endian) ─────────────────────────────────────
  // 0x00: magic [4]
  // 0x04: file creator [16]
  // 0x14: format version (uint32 BE)
  // 0x18: record directory offset (uint32 BE)
  // 0x1c: record directory entry count (uint32 BE)
  // 0x20: next free record ID (uint32 BE)
  const idxOff = view.getUint32(0x18, false);
  const nEntries = view.getUint32(0x1c, false);

  if (nEntries > 100 || idxOff + nEntries * 32 > len) {
    throw new Error('Invalid AFF record directory');
  }

  // ── Parse record directory entries ─────────────────────────────────────
  // Each entry is 32 bytes (big-endian):
  //   0x00: record type (uint16)
  //   0x02: record subtype (uint16)
  //   0x04: record version (uint32)
  //   0x08: record ID (uint32)
  //   0x0c: record data offset (uint32)
  //   0x10: record data length (uint32)
  //   0x14: parent ID
  //   0x18: object ID
  //   0x1c: checksum

  let rawDataOff = -1;
  let rawDataLen = 0;

  for (let i = 0; i < nEntries; i++) {
    const entryOff = idxOff + i * 32;
    if (entryOff + 32 > len) break;

    const recType = view.getUint16(entryOff, false);
    const recOff = view.getUint32(entryOff + 12, false);
    const recLen = view.getUint32(entryOff + 16, false);

    // Record type 1 = raw thermal data frame
    if (recType === 1 && recOff > 0 && recOff + recLen <= len) {
      rawDataOff = recOff;
      rawDataLen = recLen;
    }
  }

  if (rawDataOff < 0) {
    throw new Error('No thermal data record found in AFF file');
  }

  // ── Parse frame data header ────────────────────────────────────────────
  // AFF1 record layout (varies by camera; SC2000-style documented here):
  //
  // Byte order is determined by the first uint16: 0x02 = little-endian
  // (exiftool checks: if val >= 0x100, byte order is wrong → toggle)
  const byteOrderMarker = view.getUint16(rawDataOff, true);
  const isLE = byteOrderMarker < 0x0100; // 2 = LE, else swapped

  // Re-read with correct endianness if needed
  const get16 = (off: number): number => {
    return view.getUint16(off, isLE);
  };
  const getF32 = (off: number): number => {
    return view.getFloat32(off, isLE);
  };
  const getI32 = (off: number): number => {
    return view.getInt32(off, isLE);
  };

  // SC2000-style sensor dimensions at +0x02, +0x04
  const sensorW = get16(rawDataOff + 2);
  const sensorH = get16(rawDataOff + 4);

  // Crop region at +0x0a..+0x10 (relative to frame header start)
  const cropX1 = get16(rawDataOff + 10);
  const cropX2 = get16(rawDataOff + 12);
  const cropY1 = get16(rawDataOff + 14);
  const cropY2 = get16(rawDataOff + 16);

  const cropW = cropX2 - cropX1 + 1;
  const cropH = cropY2 - cropY1 + 1;

  if (cropW <= 0 || cropH <= 0 || cropW > 4096 || cropH > 4096) {
    throw new Error(`Invalid crop dimensions: ${cropW}x${cropH}`);
  }

  // ── Read calibration parameters ────────────────────────────────────────
  // PM695 / SC2000-style offsets (relative to record start):
  //   +0x2c: emissivity (float32)
  //   +0x30: object distance (float32)
  //   +0x34: reflected apparent temperature in Kelvin (float32)
  //   +0x38: field (PM695: atmospheric temperature in Kelvin)
  //   +0x3c: atmospheric temperature in Kelvin (float32)
  //   +0x40: relative humidity as fraction 0-1 (float32)
  //   +0x44: Planck R1 (float32)
  //   +0x48: Planck B (float32)
  //   +0x4c: Planck F (float32)
  //   +0x50: ATA1 (float32)
  //   +0x54: ATA2 (float32)
  //   +0x58: ATB1 (float32)
  //   +0x5c: ATB2 (float32)
  //   +0x60: ATX (float32)
  //   +0x64: camera temperature range max (K, float32)
  //   +0x68: camera temperature range min (K, float32)
  //
  //   +0x10c: Planck O (int32) — later format versions use +0x16c
  //   +0x110: Planck R2 (float32) — later format versions use +0x170

  const emissivity = getF32(rawDataOff + 0x2c);
  const distance = getF32(rawDataOff + 0x30);
  const refTempK = getF32(rawDataOff + 0x34);
  const field38K = getF32(rawDataOff + 0x38);
  const atmosTempK = getF32(rawDataOff + 0x3c);
  const humidityFrac = getF32(rawDataOff + 0x40);

  const planckR1 = getF32(rawDataOff + 0x44);
  const planckB = getF32(rawDataOff + 0x48);
  const planckF = getF32(rawDataOff + 0x4c);

  // Try multiple positions for O and R2 (varies by camera version)
  // SC2000: O at +0x10c (int32), R2 at +0x110 (float)
  // Later: O at +0x16c (int32), R2 at +0x170 (float)
  // Some PM695: O at +0x10c, R2 at +0x110
  let planckO: number = getI32(rawDataOff + 0x10c);
  let planckR2: number = getF32(rawDataOff + 0x110);

  // Validate: if R2 is clearly wrong (NaN, 0, or 1.0 with no useful O), try alternative
  if (isNaN(planckR2) || planckR2 === 0 || (planckR2 === 1.0 && planckO > -100)) {
    const altO = getI32(rawDataOff + 0x16c);
    const altR2 = getF32(rawDataOff + 0x170);
    if (!isNaN(altR2) && altR2 !== 0) {
      planckO = altO;
      planckR2 = altR2;
    }
  }

  // ── Read raw pixel data ────────────────────────────────────────────────
  // Pixel data starts at +0x298 from record start.
  // Stored at full sensor stride (sensorW), cropped to (cropW × cropH).
  const pixelDataOff = rawDataOff + 0x298;

  const pixelCount = cropW * cropH;
  const pixelBytesNeeded = sensorW * sensorH * 2;

  if (pixelDataOff + pixelBytesNeeded > len) {
    throw new Error('Raw pixel data truncated in AFF file');
  }

  // ── Convert raw values to Celsius ──────────────────────────────────────
  // PM695 / older cameras: raw values represent centi-Kelvin
  // (i.e., pixel value / 100 = Kelvin, Kelin - 273.15 = Celsius)
  //
  // The raw2temp Planck formula is also applied for accurate radiometry,
  // but for the classic PM695 the simplified centi-Kelvin interpretation
  // produces more accurate results because the calibration constants (O, R2)
  // are not always present or correct in the file.
  //
  // Strategy: use centi-Kelvin as the primary mapping, since PM695 raw
  // values are pre-calibrated to temperature. For cameras where this
  // produces unreasonable results, fall through to the Planck formula.

  const celsius = new Float32Array(pixelCount);
  let dataMin = Infinity;
  let dataMax = -Infinity;
  let minIdx = 0;
  let maxIdx = 0;

  // Try centi-Kelvin conversion first
  let useCentiKelvin = false;

  // Read a few sample pixels to check if centi-Kelvin is reasonable
  const sampleRaw: number[] = [];
  for (let s = 0; s < Math.min(20, pixelCount); s++) {
    const y = cropY1 + Math.floor((s / cropW) % cropH);
    const x = cropX1 + (s % cropW);
    sampleRaw.push(get16(pixelDataOff + (y * sensorW + x) * 2));
  }
  const sampleMin = Math.min(...sampleRaw);
  const sampleMax = Math.max(...sampleRaw);
  const sampleCKmin = sampleMin / 100 - 273.15;
  const sampleCKmax = sampleMax / 100 - 273.15;

  // Heuristic: centi-Kelvin is reasonable if min > -50°C and max < 2000°C
  if (sampleCKmin > -50 && sampleCKmax < 2000) {
    useCentiKelvin = true;
  }

  if (useCentiKelvin) {
    // Direct centi-Kelvin → Celsius
    // Data stored at full sensor stride (sensorW), cropped to (cropW × cropH)
    for (let pi = 0; pi < pixelCount; pi++) {
      const y = cropY1 + Math.floor(pi / cropW);
      const x = cropX1 + (pi % cropW);
      const raw = get16(pixelDataOff + (y * sensorW + x) * 2);
      const c = raw / 100 - 273.15;
      celsius[pi] = c;
      if (c < dataMin) { dataMin = c; minIdx = pi; }
      if (c > dataMax) { dataMax = c; maxIdx = pi; }
    }
  } else {
    // Full Planck formula conversion
    // Data stored at full sensor stride (sensorW)
    const allRaw = new Float32Array(pixelCount);
    for (let pi = 0; pi < pixelCount; pi++) {
      const y = cropY1 + Math.floor(pi / cropW);
      const x = cropX1 + (pi % cropW);
      allRaw[pi] = get16(pixelDataOff + (y * sensorW + x) * 2);
    }

    const cameras = { // default calibrations from Thermimage
      R1: planckR1 || 21106.77,
      B: planckB || 1501,
      F: planckF || 1,
      O: planckO || -7340,
      R2: planckR2 || 0.012545258,
    };

    for (let pi = 0; pi < pixelCount; pi++) {
      const c = raw2tempCelsius(
        allRaw[pi],
        emissivity || 1,
        distance || 1,
        refTempK - 273.15,
        atmosTempK - 273.15,
        atmosTempK - 273.15,
        1.0,
        humidityFrac * 100,
        cameras.R1, cameras.B, cameras.F, cameras.O, cameras.R2,
      );
      celsius[pi] = c;
      if (c < dataMin) { dataMin = c; minIdx = pi; }
      if (c > dataMax) { dataMax = c; maxIdx = pi; }
    }
  }

  // ── Build CDF lookup for histogram equalization ────────────────────────
  const cdfLut = buildCDF(celsius);

  // ── Extract camera metadata from header text fields ────────────────────
  let cameraModel = '';
  let cameraSerial = '';
  let fileTimestamp: string | null = null;

  // Camera model at offset 0x134 in file header (null-terminated string)
  try {
    let modelStr = '';
    for (let m = 0; m < 32; m++) {
      const ch = view.getUint8(0x134 + m);
      if (ch === 0) break;
      modelStr += String.fromCharCode(ch);
    }
    cameraModel = modelStr;
  } catch {
    // ignore
  }

  // Serial number at offset 0x148
  try {
    let serialStr = '';
    for (let s = 0; s < 16; s++) {
      const ch = view.getUint8(0x148 + s);
      if (ch === 0) break;
      serialStr += String.fromCharCode(ch);
    }
    cameraSerial = serialStr;
  } catch {
    // ignore
  }

  // Date/time at offset 0x1b4
  try {
    let dateStr = '';
    for (let d = 0; d < 18; d++) {
      const ch = view.getUint8(0x1b4 + d);
      if (ch === 0) break;
      dateStr += String.fromCharCode(ch);
    }
    if (dateStr.trim()) {
      fileTimestamp = dateStr;
    }
  } catch {
    // ignore
  }

  return {
    width: cropW,
    height: cropH,
    celsius,
    dataMin,
    dataMax,
    cdfLut,
    minSpot: { x: minIdx % cropW, y: Math.floor(minIdx / cropW), tempC: dataMin },
    maxSpot: { x: maxIdx % cropW, y: Math.floor(maxIdx / cropW), tempC: dataMax },
    coarseData: new Uint8Array(0),
    emissivity: emissivity || 0.95,
    airTemp: (atmosTempK > 0 ? atmosTempK - 273.15 : 20),
    distance: distance || 1,
    humidity: (humidityFrac > 0 && humidityFrac <= 1 ? humidityFrac * 100 : 50),
    refTemp: (refTempK > 0 ? refTempK - 273.15 : 20),
    atmTrans: calculateTau(
      distance || 1,
      atmosTempK - 273.15,
      humidityFrac * 100,
    ),
    fileName: '',
    fileModified: fileTimestamp ? Date.parse(fileTimestamp) : null,
  };
}

/**
 * Planck-based raw-to-temperature conversion.
 *
 * Ported from the Thermimage R package's raw2temp function, which is
 * itself based on the exiftool forum discussion:
 *   http://130.15.24.88/exiftool/forum/index.php/topic,4898.60.html
 * and Minkina & Dudzik's "Infrared Thermography: Errors and Uncertainties".
 */
function raw2tempCelsius(
  raw: number,
  E: number,
  OD: number,
  RTemp: number,
  ATemp: number,
  IRWTemp: number,
  IRT: number,
  RH: number,
  PR1: number,
  PB: number,
  PF: number,
  PO: number,
  PR2: number,
): number {
  const ABS_ZERO = 273.15;

  // Constants from FLIR calibration
  const ATA1 = 0.006569;
  const ATA2 = 0.01262;
  const ATB1 = -0.002276;
  const ATB2 = -0.00667;
  const ATX = 1.9;

  // Transmission through window (calibrated)
  const emissWind = 1 - IRT;
  const reflWind = 0; // anti-reflective coating

  // Water vapor pressure / transmission through atmosphere
  const h2o = (RH / 100) * Math.exp(
    1.5587 + 0.06939 * ATemp - 0.00027816 * ATemp ** 2 + 0.00000068455 * ATemp ** 3,
  );
  const tau1 =
    ATX * Math.exp(-Math.sqrt(OD / 2) * (ATA1 + ATB1 * Math.sqrt(h2o))) +
    (1 - ATX) * Math.exp(-Math.sqrt(OD / 2) * (ATA2 + ATB2 * Math.sqrt(h2o)));
  const tau2 = tau1; // same formula for both path halves

  // Helper: radiance in raw units from temperature and Planck constants
  const radiance = (tempC: number): number => {
    const T = tempC + ABS_ZERO;
    const denom = PR2 * (Math.exp(PB / T) - PF);
    if (denom <= 0) return 1e6;
    return PR1 / denom - PO;
  };

  // Radiance from the environment components
  const rawRefl1 = radiance(RTemp);
  const rawRefl1Attn = ((1 - E) / E) * rawRefl1;

  const rawAtm1 = radiance(ATemp);
  const rawAtm1Attn = ((1 - tau1) / E / tau1) * rawAtm1;

  const rawWind = radiance(IRWTemp);
  const rawWindAttn = (emissWind / E / tau1 / IRT) * rawWind;

  const rawRefl2 = radiance(RTemp);
  const rawRefl2Attn = (reflWind / E / tau1 / IRT) * rawRefl2; // ~0 due to reflWind=0

  const rawAtm2 = radiance(ATemp);
  const rawAtm2Attn = ((1 - tau2) / E / tau1 / IRT / tau2) * rawAtm2;

  // Object radiance in raw units
  const rawObj =
    raw / E / tau1 / IRT / tau2 -
    rawAtm1Attn -
    rawAtm2Attn -
    rawWindAttn -
    rawRefl1Attn -
    rawRefl2Attn;

  // Temperature from radiance via inverted Planck
  const val = PR1 / (PR2 * (rawObj + PO)) + PF;
  if (val <= 1) return -273.15; // degenerate case
  return PB / Math.log(val) - ABS_ZERO;
}

/**
 * Calculate atmospheric transmission (tau).
 * Uses the same formula as the full raw2temp chain.
 */
function calculateTau(
  distance: number,
  atmosTempC: number,
  humidity: number,
): number {
  const ATA1 = 0.006569;
  const ATA2 = 0.01262;
  const ATB1 = -0.002276;
  const ATB2 = -0.00667;
  const ATX = 1.9;

  const h2o = (humidity / 100) * Math.exp(
    1.5587 + 0.06939 * atmosTempC - 0.00027816 * atmosTempC ** 2 + 0.00000068455 * atmosTempC ** 3,
  );
  return (
    ATX * Math.exp(-Math.sqrt(distance / 2) * (ATA1 + ATB1 * Math.sqrt(h2o))) +
    (1 - ATX) * Math.exp(-Math.sqrt(distance / 2) * (ATA2 + ATB2 * Math.sqrt(h2o)))
  );
}

/** Build a cumulative-distribution LUT from a Celsius grid (1024 bins). */
function buildCDF(celsius: Float32Array): Float32Array {
  const BINS = 1024;
  let dmin = Infinity;
  let dmax = -Infinity;
  for (let i = 0; i < celsius.length; i++) {
    if (celsius[i] < dmin) dmin = celsius[i];
    if (celsius[i] > dmax) dmax = celsius[i];
  }
  const span = dmax - dmin || 1;
  const hist = new Uint32Array(BINS);
  for (let i = 0; i < celsius.length; i++) {
    const bin = Math.floor(((celsius[i] - dmin) / span) * (BINS - 1));
    hist[Math.max(0, Math.min(BINS - 1, bin))]++;
  }
  const lut = new Float32Array(BINS);
  let acc = 0;
  for (let i = 0; i < BINS; i++) {
    acc += hist[i];
    lut[i] = acc / celsius.length;
  }
  return lut;
}
