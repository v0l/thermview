import { parseIRG } from '@/lib/irg-parser';
import { parseHikmicro } from '@/lib/hikmicro-parser';
import { parseDJI } from '@/lib/dji-parser';
import { parseFLIR } from '@/lib/flir-parser';
import { parseFLIRRJPEG } from '@/lib/flir-rjpeg-parser';
import type { ThermalImage } from '@/lib/types';

/** Supported file format identifiers */
export type ThermalFormat = 'irg' | 'hikmicro' | 'dji' | 'flir' | 'flir-rjpeg';

/**
 * Auto-detect the thermal image format from raw bytes and parse it.
 *
 * Detection is based on file magic bytes and structural markers:
 *   - IRG: starts with known magic bytes (0xCAAC, 0x04A0, or tail 0xCAAC)
 *   - Hikmicro: contains HDRI block after JPEGs
 *   - DJI: contains DJI APP3 thermal chunks + APP4 calibration
 */
export function parseThermalImage(buffer: ArrayBuffer, fileName?: string, modified?: number | null): ThermalImage {
  const bytes = new Uint8Array(buffer);
  const format = detectFormat(bytes, fileName);

  let image: ThermalImage;
  switch (format) {
    case 'irg':
      image = parseIRG(buffer);
      break;
    case 'hikmicro':
      image = parseHikmicro(buffer);
      break;
    case 'dji':
      image = parseDJI(buffer);
      break;
    case 'flir':
      image = parseFLIR(buffer);
      break;
    case 'flir-rjpeg':
      image = parseFLIRRJPEG(buffer, fileName, modified);
      break;
    default:
      throw new Error(`Unsupported file format${fileName ? ': ' + fileName : ''}`);
  }

  image.fileName = fileName || '';
  image.fileModified = modified ?? null;

  return image;
}

/**
 * Detect the thermal image format from raw bytes.
 */
function detectFormat(bytes: Uint8Array, fileName?: string): ThermalFormat {
  // ── Check for FLIR AFF/FFF format (.IMG, .SEQ) ────────────────────
  if (bytes.length >= 4) {
    const magic = String.fromCharCode(bytes[0], bytes[1], bytes[2]);
    if (magic === 'AFF' || magic === 'FFF') return 'flir';
  }

  // ── Check for IRG format ──────────────────────────────────────────
  // IRG magic: offset 0-1 = known patterns, offset headerLen-2 = 0xCAAC tail
  if (bytes.length >= 4) {
    const magicLo = bytes[0];
    const magicHi = bytes[1];

    // C200/C210: 0xCA 0xAC
    if (magicLo === 0xCA && magicHi === 0xAC) return 'irg';
    // Other with CAAC tail: magic 0xBA 0xAB
    if (magicLo === 0xBA && magicHi === 0xAB) return 'irg';
    // P200: magic 0x04 0xA0
    if (magicLo === 0x04 && magicHi === 0xA0) return 'irg';

    // Check for CAAC tail (other IRG variants)
    const headerLen = (bytes[2] | (bytes[3] << 8));
    if (headerLen > 0 && headerLen < bytes.length) {
      if (bytes[headerLen - 2] === 0xAC && bytes[headerLen - 1] === 0xCA) {
        return 'irg';
      }
    }
  }

  // ── Check for JPEG-based formats ───────────────────────────────────
  // ── Check for FLIR radiometric JPEG (FLIR APP1 marker) ─────────────
  if (bytes[0] === 0xFF && bytes[1] === 0xD8) {
    // Check for FLIR APP1 marker
    let jpos = 0;
    while (jpos < bytes.length - 8) {
      if (bytes[jpos] === 0xFF && bytes[jpos + 1] === 0xE1) {
        const app1hdr = String.fromCharCode(
          bytes[jpos + 4], bytes[jpos + 5], bytes[jpos + 6], bytes[jpos + 7],
        );
        if (app1hdr === 'FLIR') return 'flir-rjpeg';
      }
      jpos++;
    }

    // JPEG — check for Hikmicro HDRI block or DJI APP3 markers

    // Check for Hikmicro: look for "HDRI" after JPEG EOI
    let offset = 0;
    while (offset < bytes.length - 4) {
      if (bytes[offset] === 0xFF && bytes[offset + 1] === 0xD9) {
        offset += 2;
        if (offset + 4 <= bytes.length) {
          const peek = String.fromCharCode(
            bytes[offset], bytes[offset + 1],
            bytes[offset + 2], bytes[offset + 3],
          );
          if (peek === 'HDRI') return 'hikmicro';
        }
        // If there's another JPEG after EOI, skip it and check after
        if (offset + 2 <= bytes.length &&
            bytes[offset] === 0xFF && bytes[offset + 1] === 0xD8) {
          // Skip through this second JPEG
          let jpegOffset = offset + 2;
          while (jpegOffset < bytes.length - 2) {
            if (bytes[jpegOffset] === 0xFF && bytes[jpegOffset + 1] === 0xD9) {
              jpegOffset += 2;
              if (jpegOffset + 4 <= bytes.length) {
                const peek2 = String.fromCharCode(
                  bytes[jpegOffset], bytes[jpegOffset + 1],
                  bytes[jpegOffset + 2], bytes[jpegOffset + 3],
                );
                if (peek2 === 'HDRI') return 'hikmicro';
              }
              break;
            }
            jpegOffset++;
          }
        }
      }
      offset++;
    }

    // Check for DJI: look for APP3 (0xFFE3) + APP4 (0xFFE4) with thermal data
    let hasApp3 = false;
    let hasApp4 = false;
    offset = 2; // skip SOI
    while (offset < bytes.length - 4) {
      if (bytes[offset] !== 0xFF) { offset++; continue; }
      const marker = bytes[offset + 1];
      if (marker === 0xFF) { offset++; continue; }
      if (marker === 0xD9 || marker === 0xDA) break;
      const len = (bytes[offset + 2] << 8) | bytes[offset + 3];
      if (marker === 0xE3 && len > 1000) hasApp3 = true;
      else if (marker === 0xE4) hasApp4 = true;
      offset += 2 + len;
    }

    if (hasApp3 && hasApp4) return 'dji';
  }

  // ── Fallback to extension-based detection ──────────────────────────
  if (fileName) {
    const ext = fileName.toLowerCase().split('.').pop();
    if (ext === 'irg') return 'irg';
    if (ext === 'img' || ext === 'seq') return 'flir';
  }

  throw new Error('Unknown thermal image format. This file contains no recognisable radiometric data.');
}