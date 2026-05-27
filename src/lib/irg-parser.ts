import type { ThermalImage } from '@/lib/types';

/**
 * Parse an Infiray IRG file into a format-agnostic ThermalImage.
 *
 * Supported variants:
 *   - C200/C210 (magic 0xACCA, flag0=0 → 1/16 K steps)
 *   - Autel Evo II Dual (magic 0xACCA, flag0=1 → 1/10 K steps)
 *   - P200 (magic 0x04A0)
 *   - "other" (magic 0xBAAB → 1/10 K steps)
 */
export function parseIRG(buffer: ArrayBuffer): ThermalImage {
  const view = new DataView(buffer);

  // --- Variable-length header ---
  const headerLen = view.getUint16(2, true);
  if (buffer.byteLength < headerLen) throw new Error('File truncated: header extends past EOF');

  // --- Detect model via magic + tail ---
  // All known variants have a 0xCAAC little-endian tail (bytes 0xAC, 0xCA)
  const magicLo = view.getUint8(0);
  const magicHi = view.getUint8(1);
  const tailLo = view.getUint8(headerLen - 2);
  const tailHi = view.getUint8(headerLen - 1);
  const hasCAACTail = tailLo === 0xAC && tailHi === 0xCA;
  const isC20x = magicLo === 0xCA && magicHi === 0xAC && hasCAACTail;
  const isOther = magicLo === 0xBA && magicHi === 0xAB && hasCAACTail;
  const isP200  = magicLo === 0x04 && magicHi === 0xA0;

  if (!isC20x && !isOther && !isP200 && !hasCAACTail) {
    throw new Error(`Unknown IRG magic: 0x${magicLo.toString(16).padStart(2,'0')}${magicHi.toString(16).padStart(2,'0')}`);
  }

  // --- Header fields (offsets 4..33) ---
  const coarseLen  = view.getUint32(4, true);   // should equal W*H
  const height     = view.getUint16(8, true);
  const width      = view.getUint16(10, true);
  const flag0      = view.getUint16(12, true);
  // const _unk1      = view.getUint16(14, true);
  // const _zero1     = view.getUint16(16, true);
  // const fineOff    = view.getUint16(18, true);
  // const _unk2      = view.getUint16(20, true);
  const jpegLen    = view.getUint32(22, true);
  // const yRes2      = view.getUint16(26, true);
  // const xRes2      = view.getUint16(28, true);
  const emissivity = view.getUint16(30, true) / 10000;
  // const fineTempOff1 = view.getUint32(34, true);
  // const fineTempOff2 = view.getUint32(38, true);
  const distance   = view.getUint32(42, true) / 10000;
  // const unitFlag  = view.getUint16(76, true);

  if (width * height !== coarseLen) {
    throw new Error(`Resolution mismatch: ${width}×${height} ≠ ${coarseLen}`);
  }

  // --- Temperature encoding ---
  // C20x flag0=0 → 1/16 K, flag0=1 → 1/10 K
  // Other known variants → 1/10 K
  // Unknown variants with 0xCAAC tail → 1/10 K
  let tempScale: number;
  if (isC20x) {
    tempScale = (flag0 === 0) ? 16 : 10;
  } else {
    tempScale = 10;
  }

  // --- Data sections ---
  let offset = headerLen;

  // Coarse (8-bit contrast-maximized) image
  const coarseSize = width * height;
  if (offset + coarseSize > buffer.byteLength) throw new Error('File truncated in coarse image');
  const coarseData = new Uint8Array(buffer.slice(offset, offset + coarseSize));
  offset += coarseSize;

  // Fine (16-bit raw temperature) grid
  const fineSizeBytes = width * height * 2;
  if (offset + fineSizeBytes > buffer.byteLength) throw new Error('File truncated in fine temperature data');
  const rawTemp = new Uint16Array(buffer.slice(offset, offset + fineSizeBytes));
  offset += fineSizeBytes;

  // Optional JPEG (visual camera image)
  if (isC20x && jpegLen > 0 && offset + jpegLen <= buffer.byteLength) {
    offset += jpegLen;
  } else if (isOther) {
    // "other" variant: rest of file is JPEG
    offset = buffer.byteLength;
  } else if (hasCAACTail && offset < buffer.byteLength) {
    // Unknown variant with CAAC tail: consume remaining as JPEG
    offset = buffer.byteLength;
  }

  // --- Convert to Celsius ---
  const pixelCount = width * height;
  const celsius = new Float32Array(pixelCount);
  let dataMin = Infinity;
  let dataMax = -Infinity;
  let minIdx = 0;
  let maxIdx = 0;
  const K_OFFSET = 273.15;

  for (let i = 0; i < pixelCount; i++) {
    const c = (rawTemp[i] / tempScale) - K_OFFSET;
    celsius[i] = c;
    if (c < dataMin) { dataMin = c; minIdx = i; }
    if (c > dataMax) { dataMax = c; maxIdx = i; }
  }

  // --- Build CDF for histogram equalization ---
  const cdfLut = buildCDF(celsius);

  // --- Remaining header params (with sensible defaults) ---
  let airTempVal = 20;
  let refTempVal = 20;
  let humidityVal = 50;
  let atmTransVal = 1;
  try {
    refTempVal = view.getUint16(36, true) / 1000;
    airTempVal = view.getUint16(38, true) / 1000;
    humidityVal = view.getUint8(0x1D);
    atmTransVal = view.getUint16(0x2A, true) / 10000;
  } catch { /* best effort */ }

  return {
    width,
    height,
    celsius,
    dataMin,
    dataMax,
    cdfLut,
    minSpot: {
      x: minIdx % width,
      y: Math.floor(minIdx / width),
      tempC: dataMin,
    },
    maxSpot: {
      x: maxIdx % width,
      y: Math.floor(maxIdx / width),
      tempC: dataMax,
    },
    coarseData,
    emissivity,
    airTemp: airTempVal,
    distance,
    humidity: humidityVal,
    refTemp: refTempVal,
    atmTrans: atmTransVal,
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

// ─── Palettes ────────────────────────────────────────────────────────────────

export type Palette = 'iron' | 'rainbow' | 'grayscale' | 'jet' | 'inferno' | 'plasma' | 'hot' | 'arctic' | 'lava';

interface ColorStop { pos: number; r: number; g: number; b: number; }

const PALETTE_DEFS: Record<Exclude<Palette, 'rainbow'>, ColorStop[]> = {
  inferno: [
    { pos: 0,    r: 0,   g: 0,   b: 4   },
    { pos: 0.2,  r: 40,  g: 11,  b: 84  },
    { pos: 0.4,  r: 132, g: 32,  b: 107 },
    { pos: 0.6,  r: 213, g: 62,  b: 40  },
    { pos: 0.8,  r: 245, g: 138, b: 30  },
    { pos: 0.95, r: 251, g: 229, b: 46  },
    { pos: 1,    r: 252, g: 255, b: 164 },
  ],
  iron: [
    { pos: 0,    r: 0,   g: 0,   b: 0   },
    { pos: 0.15, r: 12,  g: 7,   b: 34  },
    { pos: 0.3,  r: 84,  g: 4,   b: 50  },
    { pos: 0.45, r: 168, g: 13,  b: 51  },
    { pos: 0.6,  r: 230, g: 76,  b: 30  },
    { pos: 0.75, r: 245, g: 160, b: 41  },
    { pos: 0.9,  r: 252, g: 222, b: 90  },
    { pos: 1,    r: 255, g: 255, b: 220 },
  ],
  jet: [
    { pos: 0,    r: 0,   g: 0,   b: 131 },
    { pos: 0.15, r: 0,   g: 60,  b: 170 },
    { pos: 0.35, r: 5,   g: 185, b: 205 },
    { pos: 0.5,  r: 45,  g: 215, b: 90  },
    { pos: 0.65, r: 195, g: 224, b: 30  },
    { pos: 0.8,  r: 255, g: 175, b: 0   },
    { pos: 0.9,  r: 255, g: 90,  b: 0   },
    { pos: 1,    r: 128, g: 0,   b: 0   },
  ],
  plasma: [
    { pos: 0,    r: 13,  g: 8,   b: 135 },
    { pos: 0.25, r: 126, g: 3,   b: 168 },
    { pos: 0.5,  r: 203, g: 71,  b: 121 },
    { pos: 0.75, r: 244, g: 157, b: 58  },
    { pos: 0.9,  r: 248, g: 211, b: 104 },
    { pos: 1,    r: 240, g: 249, b: 33  },
  ],
  hot: [
    { pos: 0,    r: 0,   g: 0,   b: 0   },
    { pos: 0.3,  r: 128, g: 0,   b: 0   },
    { pos: 0.6,  r: 255, g: 0,   b: 0   },
    { pos: 0.8,  r: 255, g: 128, b: 0   },
    { pos: 0.95, r: 255, g: 255, b: 0   },
    { pos: 1,    r: 255, g: 255, b: 255 },
  ],
  arctic: [
    { pos: 0,    r: 0,   g: 0,   b: 30  },
    { pos: 0.3,  r: 0,   g: 60,  b: 130 },
    { pos: 0.5,  r: 20,  g: 140, b: 200 },
    { pos: 0.7,  r: 80,  g: 210, b: 230 },
    { pos: 0.85, r: 180, g: 240, b: 245 },
    { pos: 1,    r: 240, g: 255, b: 255 },
  ],
  lava: [
    { pos: 0,    r: 0,   g: 0,   b: 0   },
    { pos: 0.2,  r: 33,  g: 0,   b: 0   },
    { pos: 0.4,  r: 130, g: 15,  b: 0   },
    { pos: 0.55, r: 220, g: 60,  b: 0   },
    { pos: 0.7,  r: 245, g: 135, b: 10  },
    { pos: 0.85, r: 255, g: 220, b: 60  },
    { pos: 1,    r: 255, g: 250, b: 190 },
  ],
  grayscale: [
    { pos: 0, r: 0, g: 0, b: 0 },
    { pos: 1, r: 255, g: 255, b: 255 },
  ],
};

function gradientLookup(stops: ColorStop[], t: number): [number, number, number] {
  const tClamp = Math.max(0, Math.min(1, t));
  if (tClamp <= stops[0].pos) return [stops[0].r, stops[0].g, stops[0].b];
  if (tClamp >= stops[stops.length - 1].pos) {
    const last = stops[stops.length - 1];
    return [last.r, last.g, last.b];
  }
  for (let i = 0; i < stops.length - 1; i++) {
    if (tClamp >= stops[i].pos && tClamp <= stops[i + 1].pos) {
      const seg = (tClamp - stops[i].pos) / (stops[i + 1].pos - stops[i].pos);
      return [
        Math.floor(stops[i].r + (stops[i + 1].r - stops[i].r) * seg),
        Math.floor(stops[i].g + (stops[i + 1].g - stops[i].g) * seg),
        Math.floor(stops[i].b + (stops[i + 1].b - stops[i].b) * seg),
      ];
    }
  }
  return [0, 0, 0];
}

function paletteColor(palette: Palette, t: number): [number, number, number] {
  if (palette === 'rainbow') {
    const r = Math.floor(Math.sin(t * Math.PI) * 255);
    const g = Math.floor(Math.sin((t - 0.33) * Math.PI) * 255);
    const b = Math.floor(Math.sin((t - 0.66) * Math.PI) * 255);
    return [Math.max(0, r), Math.max(0, g), Math.max(0, b)];
  }
  return gradientLookup(PALETTE_DEFS[palette], t);
}

// ─── Render options ──────────────────────────────────────────────────────────

export type Overscan = 'clip' | 'below' | 'above' | 'none';
export type ScaleMode = 'linear' | 'log' | 'equalize';

export interface RenderOpts {
  palette: Palette;
  minC: number;
  maxC: number;
  overscan: Overscan;
  scaleMode: ScaleMode;
  inverted: boolean;
  belowColor: [number, number, number];
  aboveColor: [number, number, number];
  cdfLut?: Float32Array;
}

// ─── Scale helpers ───────────────────────────────────────────────────────────

export function scaleTempToT(c: number, minC: number, maxC: number, mode: ScaleMode, cdfLut?: Float32Array): number {
  const span = maxC - minC || 1;
  if (c <= minC) return 0;
  if (c >= maxC) return 1;

  if (mode === 'equalize' && cdfLut && cdfLut.length > 1) {
    const t = (c - minC) / span;
    const idxF = t * (cdfLut.length - 1);
    const lo = Math.floor(idxF);
    const hi = Math.min(lo + 1, cdfLut.length - 1);
    const frac = idxF - lo;
    return cdfLut[lo] + (cdfLut[hi] - cdfLut[lo]) * frac;
  }
  if (mode === 'log') {
    const shift = minC - 1;
    const lo = Math.log(Math.max(0.001, minC - shift));
    const hi = Math.log(maxC - shift);
    const val = Math.log(c - shift);
    return (val - lo) / (hi - lo || 1);
  }

  return (c - minC) / span;
}

export function tToTempC(t: number, minC: number, maxC: number, mode: ScaleMode): number {
  const tClamp = Math.max(0, Math.min(1, t));
  if (mode === 'log') {
    const shift = minC - 1;
    const lo = Math.log(Math.max(0.001, minC - shift));
    const hi = Math.log(maxC - shift);
    return Math.exp(lo + tClamp * (hi - lo)) + shift;
  }
  return minC + tClamp * (maxC - minC);
}

// ─── Render ──────────────────────────────────────────────────────────────────

/**
 * Render a thermal image from a pre-computed Celsius grid.
 */
export function createTempCanvas(
  celsius: Float32Array,
  width: number,
  height: number,
  opts: RenderOpts,
): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d')!;
  const img = ctx.createImageData(width, height);
  const pixels = img.data;

  for (let i = 0; i < celsius.length; i++) {
    const c = celsius[i];

    const t = scaleTempToT(c, opts.minC, opts.maxC, opts.scaleMode, opts.cdfLut);
    const paletteT = opts.inverted ? 1 - t : t;

    let rgba: [number, number, number, number];
    if (c < opts.minC) {
      if (opts.overscan === 'below') {
        rgba = [...opts.belowColor, 255];
      } else if (opts.overscan === 'none') {
        rgba = [0, 0, 0, 0];
      } else {
        rgba = [...paletteColor(opts.palette, opts.inverted ? 1 : 0), 255];
      }
    } else if (c > opts.maxC) {
      if (opts.overscan === 'above') {
        rgba = [...opts.aboveColor, 255];
      } else if (opts.overscan === 'none') {
        rgba = [0, 0, 0, 0];
      } else {
        rgba = [...paletteColor(opts.palette, opts.inverted ? 0 : 1), 255];
      }
    } else {
      const [r, g, b] = paletteColor(opts.palette, paletteT);
      rgba = [r, g, b, 255];
    }

    const idx = i * 4;
    pixels[idx] = rgba[0];
    pixels[idx + 1] = rgba[1];
    pixels[idx + 2] = rgba[2];
    pixels[idx + 3] = rgba[3];
  }

  ctx.putImageData(img, 0, 0);
  return canvas;
}

// ─── Gradient strip ──────────────────────────────────────────────────────────

export function createGradientStrip(palette: Palette, height: number, scaleMode: ScaleMode = 'linear', _minC = 0, _maxC = 1): HTMLCanvasElement {
  const strip = document.createElement('canvas');
  strip.width = 1;
  strip.height = height;
  const ctx = strip.getContext('2d')!;
  for (let y = 0; y < height; y++) {
    const viewT = 1 - y / (height - 1 || 1);
    const dataT = scaleMode === 'log'
      ? (Math.exp(viewT * Math.log(2)) - 1)
      : viewT;
    const t = scaleMode === 'log' ? Math.max(0, Math.min(1, dataT)) : viewT;
    const [r, g, b] = paletteColor(palette, t);
    ctx.fillStyle = `rgb(${r},${g},${b})`;
    ctx.fillRect(0, y, 1, 1);
  }
  return strip;
}
