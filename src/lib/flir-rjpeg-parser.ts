import { decode as decodePNG } from 'fast-png';
import type { ThermalImage } from '@/lib/types';

/**
 * Parse a FLIR radiometric JPEG (R-JPEG) into ThermalImage.
 *
 * FLIR R-JPEG files embed an FFF binary structure inside the JPEG APP1
 * marker(s). Large APP1 data may be split across multiple chunks.
 *
 * Structure:
 *   1. JPEG container with 0xFFE1 (APP1) markers containing FLIR data
 *   2. Inside APP1: FFF header + record directory + records
 *   3. CameraInfo (type 32): Planck constants, emissivity, distance, etc.
 *   4. RawData (type 1): contains a PNG with raw sensor values
 *
 * The raw PNG data requires:
 *   - Byte-order swap: FLIR writes 16-bit PNG with wrong endian
 *   - Planck formula: raw → Celsius conversion
 *
 * References:
 *   - thermal_parser (SanNianYiSi)
 *   - exiftool FLIR tags — CameraInfo record layout
 *   - Thermimage R package — Planck raw2temp implementation
 */
export function parseFLIRRJPEG(
  buffer: ArrayBuffer,
  fileName?: string,
  modified?: number | null,
): ThermalImage {
  const bytes = new Uint8Array(buffer);

  // ── Locate and reassemble FLIR APP1 chunks ──────────────────────────
  const flirData = extractFLIRAPP1Data(bytes);
  if (flirData.length === 0) {
    throw new Error('No FLIR APP1 data found in JPEG');
  }

  const fff = new DataView(flirData.buffer, flirData.byteOffset, flirData.byteLength);

  // ── Parse FFF header (big-endian) ────────────────────────────────────
  const magic = String.fromCharCode(fff.getUint8(0), fff.getUint8(1), fff.getUint8(2));
  if (magic !== 'FFF' && magic !== 'AFF') {
    throw new Error(`No FFF/AFF structure in FLIR APP1 (got "${magic}")`);
  }

  const idxOff = fff.getUint32(0x18, false);
  const nEntries = fff.getUint32(0x1c, false);
  if (nEntries > 100 || idxOff + nEntries * 32 > flirData.length) {
    throw new Error('Invalid FFF record directory');
  }

  // ── Parse record directory ──────────────────────────────────────────
  interface Rec { type: number; offset: number; length: number; }
  const records: Rec[] = [];
  for (let i = 0; i < nEntries; i++) {
    const off = idxOff + i * 32;
    if (off + 32 > flirData.length) break;
    const t = fff.getUint16(off, false);
    const o = fff.getUint32(off + 12, false);
    const l = fff.getUint32(off + 16, false);
    if (t >= 1 && o > 0 && o + l <= flirData.length) {
      records.push({ type: t, offset: o, length: l });
    }
  }

  // ── Extract calibration from CameraInfo (record type 32) ────────────
  const camRec = records.find(r => r.type === 32);
  if (!camRec) throw new Error('No CameraInfo record (type 32) found in FFF');

  // CameraInfo layout for FLIR cameras with radiometric JPEG output
  // (E40, T640, AX8, B60, etc.):
  //   +0x20: Emissivity          (float32 LE)
  //   +0x24: ObjectDistance      (float32 LE)  [m]
  //   +0x28: ReflectedTemp       (float32 LE)  [K]
  //   +0x2C: AtmosphericTemp     (float32 LE)  [K]
  //   +0x30: IRWindowTemp        (float32 LE)  [K]
  //   +0x34: IRWindowTrans       (float32 LE)
  //   +0x3C: RelativeHumidity    (float32 LE)  [fraction 0-1]
  //   +0x58: PlanckR1            (float32 LE)
  //   +0x5C: PlanckB             (float32 LE)
  //   +0x60: PlanckF             (float32 LE)
  //   PlanckO/R2 are not at fixed offsets — we search for them
  const getF32 = (off: number) => fff.getFloat32(off, true);
  const base = camRec.offset;

  const emissivity = getF32(base + 0x20);
  const distance = getF32(base + 0x24);
  const refTempK = getF32(base + 0x28);
  const atmosTempK = getF32(base + 0x2C);
  const humidityFrac = getF32(base + 0x3C);
  const planckR1 = getF32(base + 0x58);
  const planckB = getF32(base + 0x5C);
  const planckF = getF32(base + 0x60);

  // ── Find PlanckO and PlanckR2 by scanning ───────────────────────────
  // PlanckO is int32, PlanckR2 is float32 immediately after.
  // They sit at variable offsets within the FFF data.
  // Strategy: search for the known int32 bit pattern: -1143 (E40), -7142 (AX8), etc.
  // We scan the FFF data for pairs of (int32, float32) that look plausible.
  const { planckO, planckR2 } = findPlanckOR2(flirData);

  // ── Extract RawData (record type 1) ────────────────────────────────
  const rawRec = records.find(r => r.type === 1);
  if (!rawRec) throw new Error('No RawData record (type 1) found in FFF');

  // Parse RawData record header for crop dimensions:
  // +0x00: byte order marker (uint16, 2=LE)
  // +0x02: width (uint16)
  // +0x04: height (uint16)
  // +0x0a: crop X1
  // +0x0c: crop X2
  // +0x0e: crop Y1
  // +0x10: crop Y2
  const rawOff = rawRec.offset;
  const byteOrderMarker = fff.getUint16(rawOff, true);
  const isLE = byteOrderMarker < 0x0100;
  const get16R = (off: number) => fff.getUint16(off, isLE);

  const cropX1 = get16R(rawOff + 10);
  const cropX2 = get16R(rawOff + 12);
  const cropY1 = get16R(rawOff + 14);
  const cropY2 = get16R(rawOff + 16);
  const cropW = cropX2 - cropX1 + 1;
  const cropH = cropY2 - cropY1 + 1;

  if (cropW <= 0 || cropH <= 0 || cropW > 4096 || cropH > 4096) {
    throw new Error(`Invalid crop dimensions: ${cropW}x${cropH}`);
  }

  // ── Extract thermal PNG ──────────────────────────────────────────────
  // PNG starts at +0x20 for newer cameras, but sometimes elsewhere.
  // Scan for PNG magic in the record.
  const thermalPng = extractPNGFromSlice(
    flirData.slice(rawOff, rawOff + rawRec.length)
  ) ?? flirData.slice(rawOff + 0x20, rawOff + rawRec.length);

  // ── Decode PNG ───────────────────────────────────────────────────────
  const rawValues = decodePNG16ToUint16(thermalPng);

  // ── Fix byte order ───────────────────────────────────────────────────
  // FLIR stores 16-bit PNG values in wrong byte order.
  // Reference: thermal_parser/thermal.py parse_raw_data() line 297
  for (let i = 0; i < rawValues.length; i++) {
    const v = rawValues[i];
    rawValues[i] = ((v >> 8) | ((v & 0xFF) << 8)) & 0xFFFF;
  }

  // ── Convert to Celsius ──────────────────────────────────────────────────
  // All R-JPEG cameras carry Planck constants in CameraInfo (type 32).
  // The centi-Kelvin path only applies to ancient binary AFF cameras (PM695);
  // R-JPEG raw values are always sensor AD counts using the Planck formula.
  const refTempC = (refTempK > 0 ? refTempK - 273.15 : 20);
  const atmosTempC = (atmosTempK > 0 ? atmosTempK - 273.15 : 20);
  const humidity = (humidityFrac > 0 && humidityFrac <= 1 ? humidityFrac * 100 : 50);

  const pixelCount = rawValues.length;
  const celsius = new Float32Array(pixelCount);
  let dataMin = Infinity;
  let dataMax = -Infinity;
  let minIdx = 0;
  let maxIdx = 0;

  const r1 = planckR1 || 21106.77;
  const B = planckB || 1501;
  const F = planckF || 1;
  const O = planckO ?? -7340;
  const r2 = planckR2 || 0.012545258;

  for (let i = 0; i < pixelCount; i++) {
    const c = raw2tempCelsius(
      rawValues[i],
      emissivity || 0.95, distance || 1,
      refTempC, atmosTempC, atmosTempC, 1.0, humidity,
      r1, B, F, O, r2,
    );
    celsius[i] = c;
    // Filter absolute-zero pixels (blocked/masked sensor elements)
    if (c > -273.1 && c < dataMin) { dataMin = c; minIdx = i; }
    if (c > dataMax) { dataMax = c; maxIdx = i; }
  }



  const cdfLut = buildCDF(celsius);
  const atmT = calculateTau(distance || 1, atmosTempC, humidity);

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
    airTemp: atmosTempC,
    distance: distance || 1,
    humidity,
    refTemp: refTempC,
    atmTrans: atmT,
    fileName: fileName || '',
    fileModified: modified ?? null,
    isRecomputable: true,
    rawValues: rawValues.slice(),
    planckR1: planckR1 || 21106.77,
    planckB: planckB || 1501,
    planckF: planckF || 1,
    planckO: planckO ?? -7340,
    planckR2: planckR2 || 0.012545258,
  };
}

// ──────────────── Chunked APP1 reassembly ────────────────────────────────

/**
 * Extract and reassemble FLIR APP1 data.
 *
 * Chunk header (8 bytes after length field):
 *   "FLIR\0" (5) | reserved (1) | chunk_nr (1) | chunk_count (1)
 */
function extractFLIRAPP1Data(bytes: Uint8Array): Uint8Array {
  const chunks: { [nr: number]: Uint8Array } = {};
  let totalChunks = -1;
  let offset = 2; // skip JPEG SOI
  const HDR_LEN = 8;

  while (offset < bytes.length - 12) {
    if (bytes[offset] !== 0xFF) { offset++; continue; }
    const marker = bytes[offset + 1];
    if (marker === 0xFF) { offset++; continue; }
    if (marker === 0xD9 || marker === 0xDA) break;

    const length = (bytes[offset + 2] << 8) | bytes[offset + 3];

    if (marker === 0xE1 && length >= HDR_LEN + 1) {
      if (bytes[offset + 4] === 0x46 && bytes[offset + 5] === 0x4C &&
          bytes[offset + 6] === 0x49 && bytes[offset + 7] === 0x52) {
        const dataLen = length - 2 - HDR_LEN;
        const chunkNr = bytes[offset + 4 + 6];   // magic(5) + reserved(1)
        const total = bytes[offset + 4 + 7];
        const dataStart = offset + 4 + HDR_LEN;

        if (totalChunks < 0) totalChunks = total;
        if (chunks[chunkNr]) throw new Error(`Duplicate FLIR chunk #${chunkNr}`);
        chunks[chunkNr] = bytes.slice(dataStart, dataStart + dataLen);
      }
    }
    offset += 2 + length;
  }

  if (totalChunks <= 0 || Object.keys(chunks).length === 0) {
    return new Uint8Array(0);
  }

  const result: number[] = [];
  for (let i = 0; i <= totalChunks; i++) {
    if (chunks[i]) result.push(...chunks[i]);
  }
  return new Uint8Array(result);
}

// ── Find Planck O (int32) + R2 (float32) by scanning ────────────────────

function findPlanckOR2(flirData: Uint8Array): { planckO: number | null; planckR2: number | null } {
  // Planck O is a small negative int32 (-7340, -1143, -7142, etc.)
  // Planck R2 is a small positive float32 (~0.01-0.02)
  // They appear consecutively: [O: int32 LE] [R2: float32 LE]
  const view = new DataView(flirData.buffer, flirData.byteOffset, flirData.byteLength);
  const best: { o: number; r2: number }[] = [];

  for (let i = 0; i < flirData.length - 8; i++) {
    const o = view.getInt32(i, true);
    if (o > -20000 && o < -100) {
      const r2 = view.getFloat32(i + 4, true);
      if (r2 > 0.005 && r2 < 0.1) {
        best.push({ o, r2 });
      }
    }
  }

  if (best.length === 0) return { planckO: null, planckR2: null };

  // Pick the most negative O (most likely the Planck constant)
  best.sort((a, b) => a.o - b.o);
  return { planckO: best[0].o, planckR2: best[0].r2 };
}

// ── PNG scanning / extraction ─────────────────────────────────────────────

function extractPNGFromSlice(data: Uint8Array): Uint8Array | undefined {
  const PNG_SIG = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
  for (let i = 0; i < data.length - 8; i++) {
    if (data[i] !== 0x89) continue;
    let match = true;
    for (let j = 0; j < 8; j++) {
      if (data[i + j] !== PNG_SIG[j]) { match = false; break; }
    }
    if (!match) continue;
    let end = i + 8;
    while (end + 8 <= data.length) {
      if (end + 8 > data.length) break;
      const chunkLen = (data[end] << 24) | (data[end + 1] << 16) |
        (data[end + 2] << 8) | data[end + 3];
      const chunkType = String.fromCharCode(
        data[end + 4], data[end + 5], data[end + 6], data[end + 7],
      );
      const chunkTotal = 12 + chunkLen;
      if (end + chunkTotal > data.length) break;
      end += chunkTotal;
      if (chunkType === 'IEND') break;
      if (chunkLen > 50 * 1024 * 1024) break;
    }
    return data.slice(i, end);
  }
  return undefined;
}

// ── PNG decoding via fast-png ────────────────────────────────────────────

function decodePNG16ToUint16(pngBytes: Uint8Array): Uint16Array {
  const decoded = decodePNG(pngBytes);
  if (decoded.depth !== 16 || decoded.channels !== 1) {
    throw new Error(
      `Expected 16-bit grayscale PNG, got depth=${decoded.depth} channels=${decoded.channels}`,
    );
  }
  // data is Uint16Array for 16-bit 1-channel
  return decoded.data as Uint16Array;
}

// ── Planck formula ────────────────────────────────────────────────────────

function raw2tempCelsius(
  raw: number, E: number, OD: number,
  RTemp: number, ATemp: number, IRWTemp: number, IRT: number, RH: number,
  PR1: number, PB: number, PF: number, PO: number, PR2: number,
): number {
  const ABS_ZERO = 273.15;
  const ATA1 = 0.006569;
  const ATA2 = 0.01262;
  const ATB1 = -0.002276;
  const ATB2 = -0.00667;
  const ATX = 1.9;

  const emissWind = 1 - IRT;
  const reflWind = 0;
  const h2o = (RH / 100) * Math.exp(
    1.5587 + 0.06939 * ATemp - 0.00027816 * ATemp ** 2 + 0.00000068455 * ATemp ** 3,
  );
  const sqrtOD2 = Math.sqrt(OD / 2);
  const sqrtH2O = Math.sqrt(h2o);
  const tau1 = ATX * Math.exp(-sqrtOD2 * (ATA1 + ATB1 * sqrtH2O)) +
    (1 - ATX) * Math.exp(-sqrtOD2 * (ATA2 + ATB2 * sqrtH2O));
  const tau2 = tau1;

  const radiance = (tempC: number): number => {
    const T = tempC + ABS_ZERO;
    const denom = PR2 * (Math.exp(PB / T) - PF);
    if (denom <= 0) return 1e6;
    return PR1 / denom - PO;
  };

  const rawRefl1 = radiance(RTemp);
  const rawRefl1Attn = ((1 - E) / E) * rawRefl1;
  const rawAtm1 = radiance(ATemp);
  const rawAtm1Attn = ((1 - tau1) / E / tau1) * rawAtm1;
  const rawWind = radiance(IRWTemp);
  const rawWindAttn = (emissWind / E / tau1 / IRT) * rawWind;
  const rawRefl2 = radiance(RTemp);
  const rawRefl2Attn = (reflWind / E / tau1 / IRT) * rawRefl2;
  const rawAtm2 = radiance(ATemp);
  const rawAtm2Attn = ((1 - tau2) / E / tau1 / IRT / tau2) * rawAtm2;

  const rawObj = raw / E / tau1 / IRT / tau2 -
    rawAtm1Attn - rawAtm2Attn - rawWindAttn - rawRefl1Attn - rawRefl2Attn;
  const val = PR1 / (PR2 * (rawObj + PO)) + PF;
  if (val <= 1) return -273.15;
  return PB / Math.log(val) - ABS_ZERO;
}

// ── Atmospheric transmission ──────────────────────────────────────────────

function calculateTau(distance: number, atmosTempC: number, humidity: number): number {
  const ATA1 = 0.006569, ATA2 = 0.01262, ATB1 = -0.002276, ATB2 = -0.00667, ATX = 1.9;
  const h2o = (humidity / 100) * Math.exp(
    1.5587 + 0.06939 * atmosTempC - 0.00027816 * atmosTempC ** 2 + 0.00000068455 * atmosTempC ** 3,
  );
  return ATX * Math.exp(-Math.sqrt(distance / 2) * (ATA1 + ATB1 * Math.sqrt(h2o))) +
    (1 - ATX) * Math.exp(-Math.sqrt(distance / 2) * (ATA2 + ATB2 * Math.sqrt(h2o)));
}

// ── Histogram CDF ─────────────────────────────────────────────────────────

function buildCDF(celsius: Float32Array): Float32Array {
  const BINS = 1024;
  let dmin = Infinity, dmax = -Infinity;
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
  for (let i = 0; i < BINS; i++) { acc += hist[i]; lut[i] = acc / celsius.length; }
  return lut;
}