import type { ThermalImage } from '@/lib/types';

/**
 * Parse a DJI radiometric JPEG (DTAT3.0 format) into ThermalImage.
 *
 * Supports:
 *   - DJI Mavic 2 Enterprise Advanced (M2EA)
 *   - DJI M30T, M3T, M3TD, H20N, H30T, M4T
 *
 * File structure:
 *   - SOI + APP1 (EXIF with camera metadata)
 *   - APP2 (MPF — Multi-Picture Format)
 *   - APP3 x N (raw thermal sensor data, uint16 LE, 640×512)
 *   - APP4 (calibration parameters as floats)
 *   - JPEG image data (visible/colorized)
 *   - Second JPEG (visible light image)
 *
 * Conversion uses simplified Planck-based formula:
 *   Temperature = B / ln(R1 / (R2 * (raw - O)) + F) - 273.15
 */
export function parseDJI(buffer: ArrayBuffer): ThermalImage {
  const bytes = new Uint8Array(buffer);

  // ── Extract APP3 chunks for raw thermal data ───────────────────────────
  const app3Chunks: Uint8Array[] = [];
  let app4Data: Uint8Array | null = null;

  let i = 0;
  while (i < bytes.length - 4) {
    if (bytes[i] === 0xFF) {
      const marker = bytes[i + 1];
      if (marker >= 0xE0 && marker <= 0xEF) {
        const length = (bytes[i + 2] << 8) | bytes[i + 3];
        if (length < 4) { i++; continue; }
        const contentEnd = i + 2 + length;

        if (marker === 0xE3) {
          // APP3 — raw thermal data
          const chunk = bytes.slice(i + 4, Math.min(contentEnd, bytes.length));
          app3Chunks.push(chunk);
        } else if (marker === 0xE4) {
          // APP4 — calibration parameters
          app4Data = bytes.slice(i + 4, Math.min(contentEnd, bytes.length));
        }

        i = contentEnd;
        continue;
      } else if (marker === 0xD9) {
        // EOI — stop at first JPEG end
        break;
      }
    }
    i++;
  }

  if (app3Chunks.length === 0) {
    throw new Error('No APP3 thermal data found in DJI file');
  }

  // Concatenate APP3 chunks
  const totalApp3Size = app3Chunks.reduce((s, c) => s + c.length, 0);
  const app3Data = new Uint8Array(totalApp3Size);
  let writePos = 0;
  for (const chunk of app3Chunks) {
    app3Data.set(chunk, writePos);
    writePos += chunk.length;
  }

  // ── Determine resolution ────────────────────────────────────────────
  // Standard DJI DTAT3.0 thermal resolution is 640×512, but some cameras
  // produce smaller frames. We detect resolution from data size.
  let width = 640;
  let height = 512;
  let pixelCount = width * height;

  if (app3Data.length < pixelCount * 2) {
    // Try smaller common resolutions
    const totalPixels = Math.floor(app3Data.length / 2);
    // Common DJI resolutions: 640×512, 320×256, 160×120, etc.
    if (totalPixels === 320 * 256) { width = 320; height = 256; pixelCount = totalPixels; }
    else if (totalPixels === 160 * 120) { width = 160; height = 120; pixelCount = totalPixels; }
    else {
      // Fall back to a square-ish shape
      height = Math.floor(Math.sqrt(totalPixels));
      width = Math.floor(totalPixels / height);
      pixelCount = width * height;
    }
  }

  if (app3Data.length < pixelCount * 2) {
    throw new Error(`DJI raw data too small: ${app3Data.length} < ${pixelCount * 2}`);
  }

  // ── Parse raw sensor values ─────────────────────────────────────────
  const rawView = new DataView(app3Data.buffer, app3Data.byteOffset, app3Data.byteLength);
  const rawValues = new Float64Array(pixelCount);

  for (let pi = 0; pi < pixelCount; pi++) {
    rawValues[pi] = rawView.getUint16(pi * 2, true);
  }

  // ── Parse APP4 calibration parameters ────────────────────────────────
  // APP4 contains floats: emissivity, distance, humidity, reflected_temp
  // Also sometimes contains Planck constants
  let emissivity = 0.95;
  let distance = 5.0;
  let humidity = 70.0;
  let refTemp = 23.0;

  if (app4Data && app4Data.length >= 20) {
    const calView = new DataView(app4Data.buffer, app4Data.byteOffset, app4Data.byteLength);
    // offset 0: distance (float32 LE)
    if (calView.byteLength >= 4) distance = calView.getFloat32(0, true);
    // offset 4: humidity (float32 LE)
    if (calView.byteLength >= 8) humidity = calView.getFloat32(4, true);
    // offset 8: emissivity (float32 LE)
    if (calView.byteLength >= 12) emissivity = calView.getFloat32(8, true);
    // offset 12: reflected temperature (float32 LE)
    if (calView.byteLength >= 16) refTemp = calView.getFloat32(12, true);

    // Clamp to valid ranges
    emissivity = Math.max(0.1, Math.min(1.0, emissivity));
    distance = Math.max(1, Math.min(25, distance));
    humidity = Math.max(20, Math.min(100, humidity));
    refTemp = Math.max(-40, Math.min(500, refTemp));
  }

  // ── Convert raw sensor values to Celsius ────────────────────────────
  // DJI M2EA raw values: the SDK converts to temperature internally.
  // Without the SDK, we approximate using raw/64 = Kelvin (common IR convention).
  // This gives approximate temperatures; the DJI Thermal SDK applies
  // additional calibration curves for precise radiometry.
  const K_OFFSET = 273.15;
  const celsius = new Float32Array(pixelCount);
  let dataMin = Infinity;
  let dataMax = -Infinity;
  let minIdx = 0;
  let maxIdx = 0;

  for (let pi = 0; pi < pixelCount; pi++) {
    const c = (rawValues[pi] / 64) - K_OFFSET;
    celsius[pi] = c;
    if (c < dataMin) { dataMin = c; minIdx = pi; }
    if (c > dataMax) { dataMax = c; maxIdx = pi; }
  }

  // ── Build CDF ──────────────────────────────────────────────────────
  const cdfLut = buildCDF(celsius);

  // ── Try to extract metadata from EXIF ───────────────────────────────
  let airTemp = 20;
  let atmTrans = 1;

  // Parse EXIF APP1 for camera model
  i = 0;
  while (i < bytes.length - 10) {
    if (bytes[i] === 0xFF && bytes[i + 1] === 0xE1) {
      const hdr = String.fromCharCode(...bytes.slice(i + 4, i + 10));
      if (hdr === 'Exif\x00\x00') {
        const tiffStart = i + 10;
        const tiffView = new DataView(buffer, tiffStart);
        const le = tiffView.getUint8(0) === 0x49;
        const ifdOff = tiffView.getUint32(4, le);
        const ifdAbs = tiffStart + ifdOff;
        if (ifdAbs + 2 <= buffer.byteLength) {
          const ifdView = new DataView(buffer, ifdAbs);
          const nEntries = ifdView.getUint16(0, le);
          for (let ei = 0; ei < nEntries && ei < 50; ei++) {
            const es = 2 + ei * 12;
            if (es + 12 > ifdView.byteLength) break;
            const tag = ifdView.getUint16(es, le);
            const typ = ifdView.getUint16(es + 2, le);

            // Object Distance (0x9206)
            if (tag === 0x9206 && typ === 5) {
              const voff = ifdView.getUint32(es + 8, le);
              if (tiffStart + voff + 8 <= buffer.byteLength) {
                const num = ifdView.getUint32(tiffStart + voff, le);
                const den = ifdView.getUint32(tiffStart + voff + 4, le);
                if (den !== 0) distance = num / den;
              }
            }
          }
        }
        break;
      }
    }
    i++;
  }

  return {
    width,
    height,
    celsius,
    dataMin,
    dataMax,
    cdfLut,
    minSpot: { x: minIdx % width, y: Math.floor(minIdx / width), tempC: dataMin },
    maxSpot: { x: maxIdx % width, y: Math.floor(maxIdx / width), tempC: dataMax },
    coarseData: new Uint8Array(0),
    emissivity,
    airTemp,
    distance,
    humidity,
    refTemp,
    atmTrans,
    fileName: '',
    fileModified: null,

    // DJI stores pre-calibrated temperatures, not raw sensor counts.
    isRecomputable: false,
    rawValues: null,
    planckR1: 0,
    planckB: 0,
    planckF: 0,
    planckO: 0,
    planckR2: 0,
  };
}

/** Build a cumulative-distribution LUT from a Celsius grid (1024 bins). */
function buildCDF(celsius: Float32Array): Float32Array {
  const BINS = 1024;
  let dataMin = Infinity, dataMax = -Infinity;
  for (let i = 0; i < celsius.length; i++) {
    if (celsius[i] < dataMin) dataMin = celsius[i];
    if (celsius[i] > dataMax) dataMax = celsius[i];
  }
  const span = dataMax - dataMin || 1;
  const hist = new Uint32Array(BINS);
  for (let i = 0; i < celsius.length; i++) {
    const bin = Math.floor(((celsius[i] - dataMin) / span) * (BINS - 1));
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
