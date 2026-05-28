import { useRef, useCallback, useMemo, useEffect } from 'react';
import { createTempCanvas } from '@/lib/irg-parser';
import { CURSOR_COLORS } from '@/lib/constants';
import { toUnit } from '@/lib/units';
import { drawCrosshair, drawLabel, drawMinMaxMarker, drawInfoPill } from '@/lib/drawing';
import type { MeasurementCursor, OverlayConfig, FileInfo, TempUnit, ThermalImage } from '@/lib/types';
import type { RenderOpts } from '@/lib/irg-parser';

export function ThermalCanvas({
  image, renderOpts, cursors, scale: effectiveScale, tempUnit, labelScale,
  overlay, fileInfo,
  onCursorAdd, onHover, exportRef,
}: {
  image: ThermalImage;
  renderOpts: RenderOpts;
  cursors: MeasurementCursor[];
  scale: number;
  tempUnit: TempUnit;
  labelScale: number;
  overlay: OverlayConfig;
  fileInfo: FileInfo;
  onCursorAdd: (x: number, y: number, tempC: number) => void;
  onHover: (temp: number | null) => void;
  exportRef: React.RefObject<HTMLCanvasElement | null>;
}) {
  const { width: rawW, height: rawH, celsius, emissivity, minSpot, maxSpot } = image;
  const canvasRef = useRef<HTMLCanvasElement>(null);

  const dispW = rawW * effectiveScale;
  const dispH = rawH * effectiveScale;

  const baseImage = useMemo(
    () => createTempCanvas(celsius, rawW, rawH, renderOpts),
    [celsius, rawW, rawH, renderOpts],
  );

  // Sync export ref
  useEffect(() => { exportRef.current = canvasRef.current; return () => { exportRef.current = null; }; }, [exportRef]);

  useEffect(() => {
    const cvs = canvasRef.current;
    if (!cvs) return;
    cvs.width = dispW; cvs.height = dispH;
    const ctx = cvs.getContext('2d')!;
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(baseImage, 0, 0, dispW, dispH);

    const factor = effectiveScale;
    for (const c of cursors) {
      const col = CURSOR_COLORS[c.colorIdx];
      const cx = c.x * factor + factor / 2;
      const cy = c.y * factor + factor / 2;
      const armLen = drawCrosshair(ctx, cx, cy, col, labelScale, factor);
      const lbl = `${c.label}: ${toUnit(c.tempC, tempUnit)}°${tempUnit}`;
      drawLabel(ctx, cx, cy, lbl, col, labelScale, armLen, dispW);
    }

    // Overlay: min/max markers
    if (overlay.showMinMaxSpots) {
      if (minSpot.x !== maxSpot.x || minSpot.y !== maxSpot.y) {
        drawMinMaxMarker(ctx, minSpot.x * factor + factor / 2, minSpot.y * factor + factor / 2, minSpot.tempC, 'min', tempUnit, labelScale, dispW);
      }
      drawMinMaxMarker(ctx, maxSpot.x * factor + factor / 2, maxSpot.y * factor + factor / 2, maxSpot.tempC, 'max', tempUnit, labelScale, dispW);
    }

    // Overlay: info pills (emissivity + timestamp)
    if (overlay.showEmissivity) {
      drawInfoPill(ctx, `ε=${emissivity.toFixed(2)}`, 'bottomLeft', dispW, dispH, labelScale);
    }
    if (overlay.showTimestamp && fileInfo.name) {
      const tsText = fileInfo.modified
        ? new Date(fileInfo.modified).toLocaleString(undefined, { year:'numeric', month:'short', day:'numeric', hour:'2-digit', minute:'2-digit' })
        : fileInfo.name;
      drawInfoPill(ctx, tsText, 'bottomRight', dispW, dispH, labelScale);
    }

    // Camera info pills
    const cameraInfo = image.cameraInfo;
    if (cameraInfo && overlay.showEmissivity) {
      const makeModel = [cameraInfo.make, cameraInfo.model].filter(Boolean).join(' ');
      if (makeModel) {
        drawInfoPill(ctx, makeModel, 'topLeft', dispW, dispH, labelScale);
      }
      const lensInfo = [
        cameraInfo.focalLength ? `${cameraInfo.focalLength}mm` : '',
        cameraInfo.fNumber ? `f/${cameraInfo.fNumber}` : '',
      ].filter(Boolean).join(' ');
      if (lensInfo) {
        drawInfoPill(ctx, lensInfo, 'topRight', dispW, dispH, labelScale);
      }
    }
  }, [dispW, dispH, effectiveScale, baseImage, cursors, tempUnit, labelScale, overlay, minSpot, maxSpot, emissivity, fileInfo]);

  const pickCoords = useCallback((ev: React.MouseEvent<HTMLCanvasElement>) => {
    const rect = ev.currentTarget.getBoundingClientRect();
    return {
      px: Math.floor(((ev.clientX - rect.left) / rect.width) * rawW),
      py: Math.floor(((ev.clientY - rect.top) / rect.height) * rawH),
    };
  }, [rawW, rawH]);

  const handleClick = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const { px, py } = pickCoords(e);
    if (px < 0 || py < 0 || px >= rawW || py >= rawH) return;
    onCursorAdd(px, py, celsius[py * rawW + px]);
  }, [pickCoords, rawW, rawH, celsius, onCursorAdd]);

  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const { px, py } = pickCoords(e);
    if (px < 0 || py < 0 || px >= rawW || py >= rawH) { onHover(null); return; }
    onHover(celsius[py * rawW + px]);
  }, [pickCoords, rawW, rawH, celsius, onHover]);

  return (
    <canvas
      ref={canvasRef}
      width={dispW} height={dispH}
      className="block rounded-lg border border-thermal-border cursor-crosshair"
      onClick={handleClick} onMouseMove={handleMouseMove} onMouseLeave={() => onHover(null)}
    />
  );
}
