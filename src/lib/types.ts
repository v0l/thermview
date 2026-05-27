import type { Palette, Overscan, ScaleMode } from '@/lib/irg-parser';

export type TempUnit = 'C' | 'F';

export interface MeasurementCursor {
  id: number;
  x: number;
  y: number;
  label: string;
  colorIdx: number;
  tempC: number;
}

export interface MinMaxSpot {
  x: number;
  y: number;
  tempC: number;
}

export interface OverlayConfig {
  showMinMaxSpots: boolean;
  showEmissivity: boolean;
  showTimestamp: boolean;
}

export interface FileInfo {
  name: string;
  modified: number | null;
}

/**
 * A format-agnostic decoded thermal image.
 * Once a file is parsed it becomes a ThermalImage, regardless of source format.
 */
export interface ThermalImage {
  /** Width in pixels */
  width: number;
  /** Height in pixels */
  height: number;
  /** Per-pixel Celsius temperatures – same dimensions as width×height */
  celsius: Float32Array;
  /** Pre-computed data min (°C) */
  dataMin: number;
  /** Pre-computed data max (°C) */
  dataMax: number;
  /** Pre-computed CDF lookup for histogram equalization */
  cdfLut: Float32Array;
  /** Position + value of coldest pixel */
  minSpot: MinMaxSpot;
  /** Position + value of hottest pixel */
  maxSpot: MinMaxSpot;
  /** Coarse (8-bit contrast-maximized) image – may be empty for some formats */
  coarseData: Uint8Array;
  /** Header metadata */
  emissivity: number;
  airTemp: number;
  distance: number;
  humidity: number;
  refTemp: number;
  atmTrans: number;
  /** Source file info */
  fileName: string;
  fileModified: number | null;
}

export type { Palette, Overscan, ScaleMode };
