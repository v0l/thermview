import type { ThermalImage } from '@/lib/types';

/**
 * Parse a Hikmicro / InfiRay HDRI thermal JPEG file into a format-agnostic ThermalImage.
 *
 * Supports:
 *   - Hikmicro Pocket2 (256×192)
 *   - Hikmicro SP60   (640×480)
 *
 * File structure:
 *   1. First JPEG (colorized thermal preview with Iref APP1 metadata)
 *   2. Second JPEG (visible-light camera image, if dual-camera)
 *   3. HDRI block: 44-byte header + raw 16-bit pixel data in centi-Celsius
 */
export function parseHikmicro(buffer: ArrayBuffer): ThermalImage {
  const bytes = new Uint8Array(buffer);

  // ── Find the HDRI block (after last JPEG EOI) ──────────────────────────
  let offset = 0;
  let hdriStart = -1;

  // Skip past JPEGs: find EOI markers
  while (offset < bytes.length - 2) {
    if (bytes[offset] === 0xFF && bytes[offset + 1] === 0xD9) {
      // Found EOI — check if what follows is another JPEG or HDRI
      offset += 2;
      if (offset + 4 <= bytes.length) {
        const peek = String.fromCharCode(...bytes.slice(offset, offset + 4));
        if (peek === 'HDRI') {
          hdriStart = offset;
          break;
        }
      }
    } else {
      offset++;
    }
  }

  if (hdriStart < 0) {
    throw new Error('No HDRI thermal data block found in Hikmicro file');
  }

  // ── Parse HDRI header ──────────────────────────────────────────────────
  const hdri = new DataView(buffer, hdriStart);

  // 0x00: "HDRI" magic (4 bytes)
  // 0x04: uint16 magic = 0x1028
  const magic = hdri.getUint16(4, true);
  if (magic !== 0x1028) {
    throw new Error(`Unknown HDRI magic: 0x${magic.toString(16)}`);
  }

  // 0x08: uint32 reserved (0)
  // 0x0C: uint32 width
  const width = hdri.getUint32(12, true);
  // 0x10: uint32 height
  const height = hdri.getUint32(16, true);
  // 0x14: uint32 data_size (not always accurate)

  if (width === 0 || height === 0 || width > 4096 || height > 4096) {
    throw new Error(`Invalid HDRI dimensions: ${width}×${height}`);
  }

  // ── Read raw thermal pixels ────────────────────────────────────────────
  // Pixel data starts at offset 44 within the HDRI block
  const dataOffset = hdriStart + 44;
  const pixelCount = width * height;
  const dataSize = pixelCount * 2; // uint16 LE

  if (dataOffset + dataSize > buffer.byteLength) {
    throw new Error('HDRI pixel data truncated');
  }

  const rawView = new DataView(buffer, dataOffset, dataSize);
  const celsius = new Float32Array(pixelCount);
  let dataMin = Infinity;
  let dataMax = -Infinity;
  let minIdx = 0;
  let maxIdx = 0;

  // Hikmicro stores temperatures as centi-Celsius (value / 100 = °C)
  for (let i = 0; i < pixelCount; i++) {
    const raw = rawView.getUint16(i * 2, true);
    const c = raw / 100;
    celsius[i] = c;
    if (c < dataMin) { dataMin = c; minIdx = i; }
    if (c > dataMax) { dataMax = c; maxIdx = i; }
  }

  // ── Build CDF for histogram equalization ───────────────────────────────
  const cdfLut = buildCDF(celsius);

  // ── Extract EXIF metadata from first JPEG if available ─────────────────
  let emissivity = 0.95;
  let airTemp = 20;
  let distance = 1;
  let humidity = 50;
  let refTemp = 20;
  let atmTrans = 1;

  // Try to read EXIF from the first JPEG
  // Look for APP1 Exif marker
  let exifOffset = -1;
  let searchOff = 0;
  while (searchOff < bytes.length - 4) {
    if (bytes[searchOff] === 0xFF && bytes[searchOff + 1] === 0xE1) {
      const hdr = String.fromCharCode(...bytes.slice(searchOff + 4, searchOff + 10));
      if (hdr === 'Exif\x00\x00') {
        exifOffset = searchOff + 10; // TIFF start
        break;
      }
    }
    searchOff++;
  }

  if (exifOffset >= 0) {
    const exifView = new DataView(buffer, exifOffset);
    const bo = exifView.getUint8(0) === 0x49 ? 'LE' : 'BE'; // II or MM
    const le = bo === 'LE';
    const ifdOff = exifView.getUint32(4, le);
    const ifdAbs = exifOffset + ifdOff;
    if (ifdAbs < buffer.byteLength) {
      const ifdView = new DataView(buffer, ifdAbs);
      const nEntries = ifdView.getUint16(0, le);
      for (let ei = 0; ei < nEntries; ei++) {
        const es = 2 + ei * 12;
        if (es + 12 > ifdView.byteLength) break;
        const tag = ifdView.getUint16(es, le);
        // Only handle string tags for camera model
        if (tag === 0x0110) {
          // Model — but we already know this is Hikmicro
        }

        // Embedded MakerNote would have additional params,
        // but Hikmicro uses the Iref marker for its custom metadata
      }
    }
  }

  // ── Build empty coarse data (Hikmicro doesn't provide a coarse image) ──
  const coarseData = new Uint8Array(0);

  return {
    width,
    height,
    celsius,
    dataMin,
    dataMax,
    cdfLut,
    minSpot: { x: minIdx % width, y: Math.floor(minIdx / width), tempC: dataMin },
    maxSpot: { x: maxIdx % width, y: Math.floor(maxIdx / width), tempC: dataMax },
    coarseData,
    emissivity,
    airTemp,
    distance,
    humidity,
    refTemp,
    atmTrans,
    fileName: '',
    fileModified: null,
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
