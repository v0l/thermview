import type { ThermalImage, MinMaxSpot } from '@/lib/types';
import { raw2tempCelsius, calculateTau, buildCDF } from '@/lib/flir-parser';

/**
 * Recompute a thermal image with updated calibration parameters.
 *
 * Only works for ThermalImage objects where `isRecomputable` is true
 * (i.e., FLIR R-JPEG where raw sensor AD counts + Planck constants are available).
 *
 * For non-recomputable formats, returns the original image unchanged.
 */
export function recomputeThermalImage(
  original: ThermalImage,
  params: Partial<RecomputeParams>,
): ThermalImage {
  if (!original.isRecomputable || !original.rawValues) {
    return original;
  }

  const em = params.emissivity ?? original.emissivity;
  const dist = params.distance ?? original.distance;
  const refTemp = params.refTemp ?? original.refTemp;
  const airTemp = params.airTemp ?? original.airTemp;
  const hum = params.humidity ?? original.humidity;
  const r1 = original.planckR1;
  const b = original.planckB;
  const f = original.planckF;
  const o = original.planckO;
  const r2 = original.planckR2;

  const rawValues = original.rawValues;
  const pixelCount = rawValues.length;
  const celsius = new Float32Array(pixelCount);

  let dataMin = Infinity;
  let dataMax = -Infinity;
  let minIdx = 0;
  let maxIdx = 0;

  for (let i = 0; i < pixelCount; i++) {
    const c = raw2tempCelsius(
      rawValues[i],
      em, dist,
      refTemp, airTemp, airTemp, 1.0, hum,
      r1, b, f, o, r2,
    );
    celsius[i] = c;
    if (c < dataMin) { dataMin = c; minIdx = i; }
    if (c > dataMax) { dataMax = c; maxIdx = i; }
  }

  const cdfLut = buildCDF(celsius);
  const w = original.width;

  const minSpot: MinMaxSpot = { x: minIdx % w, y: Math.floor(minIdx / w), tempC: dataMin };
  const maxSpot: MinMaxSpot = { x: maxIdx % w, y: Math.floor(maxIdx / w), tempC: dataMax };

  const atmT = calculateTau(dist, airTemp, hum);

  return {
    ...original,
    celsius,
    dataMin,
    dataMax,
    cdfLut,
    minSpot,
    maxSpot,
    emissivity: em,
    airTemp,
    distance: dist,
    humidity: hum,
    refTemp,
    atmTrans: atmT,
    // rawValues and Planck constants are preserved for future recomputation
  };
}

export interface RecomputeParams {
  emissivity: number;
  distance: number;
  refTemp: number;
  airTemp: number;
  humidity: number;
}