import type { Palette, Overscan, ScaleMode } from '@/lib/irg-parser';

export type TempUnit = 'C' | 'F';

export interface CameraInfo {
  make?: string;
  model?: string;
  software?: string;
  serialNumber?: string;
  /** Focal length in mm */
  focalLength?: number;
  /** Aperture f-number */
  fNumber?: number;
  /** GPS coordinates (decimal degrees) */
  latitude?: number;
  longitude?: number;
  /** Visible-image resolution from EXIF (may differ from thermal) */
  imageWidth?: number;
  imageHeight?: number;
}

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

  /** Camera & image metadata from EXIF (populated asynchronously) */
  cameraInfo?: CameraInfo;
  /** Original capture date from EXIF DateTimeOriginal */
  captureDate?: string;

  /** Whether the raw sensor data + Planck constants are available for recalibration.
   *  When true, editing emissivity/distance/etc. can recompute temperatures.
   *  When false, calibration is baked into the stored temperatures and params are read-only. */
  isRecomputable: boolean;
  /** Raw sensor AD counts (only present when isRecomputable). Same dimensions as celsius. */
  rawValues: Uint16Array | null;
  /** Planck calibration constants for recomputation (only when isRecomputable) */
  planckR1: number;
  planckB: number;
  planckF: number;
  planckO: number;
  planckR2: number;
}

export type { Palette, Overscan, ScaleMode };

/**
 * Extract standard EXIF metadata from a binary buffer.
 * Returns CameraInfo and capture date. Does not read FLIR/DJI proprietary segments.
 */
export async function extractExifMeta(buffer: ArrayBuffer): Promise<{ cameraInfo: CameraInfo; captureDate?: string } | null> {
  try {
    const { default: exifr } = await import('exifr');
    const tags = await exifr.parse(buffer, [
      'Make', 'Model', 'Software', 'SerialNumber',
      'FocalLength', 'FNumber',
      'DateTimeOriginal', 'CreateDate',
      'latitude', 'longitude',
      'ExifImageWidth', 'ExifImageHeight',
    ]);
    if (!tags) return null;
    return {
      cameraInfo: {
        make: tags.Make,
        model: tags.Model,
        software: tags.Software,
        serialNumber: tags.SerialNumber,
        focalLength: tags.FocalLength,
        fNumber: tags.FNumber,
        latitude: tags.latitude,
        longitude: tags.longitude,
        imageWidth: tags.ExifImageWidth,
        imageHeight: tags.ExifImageHeight,
      },
      captureDate: tags.DateTimeOriginal || tags.CreateDate,
    };
  } catch {
    return null;
  }
}
