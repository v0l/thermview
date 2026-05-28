import { useState, useRef, useCallback, useMemo, useEffect } from 'react';
import { createGradientStrip, scaleTempToT, tToTempC } from '@/lib/irg-parser';
import { toUnit } from '@/lib/units';
import type { Palette, ScaleMode, TempUnit } from '@/lib/types';

export function RangeColorBar({
  palette, scaleMode, dataMin, dataMax, rangeMin, rangeMax, tempUnit, height, inverted, onMinChange, onMaxChange,
}: {
  palette: Palette;
  scaleMode: ScaleMode;
  dataMin: number; dataMax: number;
  rangeMin: number; rangeMax: number;
  tempUnit: TempUnit;
  height: number;
  inverted: boolean;
  onMinChange: (v: number) => void;
  onMaxChange: (v: number) => void;
}) {
  const [dragging, setDragging] = useState<'min' | 'max' | null>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  const tempToFrac = useCallback(
    (c: number) => scaleTempToT(c, dataMin, dataMax, scaleMode),
    [dataMin, dataMax, scaleMode],
  );
  const fracToTemp = useCallback(
    (frac: number) => tToTempC(Math.max(0, Math.min(1, frac)), dataMin, dataMax, scaleMode),
    [dataMin, dataMax, scaleMode],
  );

  const rangeMinFrac = tempToFrac(rangeMin);
  const rangeMaxFrac = tempToFrac(rangeMax);

  const dataStrip = useMemo(
    () => createGradientStrip(palette, Math.max(height, 100), scaleMode, dataMin, dataMax),
    [palette, height, scaleMode, dataMin, dataMax],
  );

  const ticks = useMemo(() => {
    const span = rangeMax - rangeMin;
    const step = span > 20 ? 5 : span > 10 ? 2 : span > 5 ? 1 : span > 2 ? 0.5 : 0.2;
    const r: { y: number; label: string }[] = [];
    for (let v = Math.ceil(rangeMin / step) * step; v <= rangeMax; v += step) {
      r.push({ y: tempToFrac(v), label: `${Math.round(v)}°` });
    }
    return r;
  }, [rangeMin, rangeMax, tempToFrac]);

  const fmt = useCallback((c: number) => `${toUnit(c, tempUnit)}`, [tempUnit]);

  // Layout: [data range labels] [gradient bar] [tick labels] [handle values + unit]
  const LEFT_W = 50;
  const BAR_W = 18;
  const TICK_W = 46;
  const RIGHT_W = 50;
  const CANVAS_W = LEFT_W + BAR_W + TICK_W + RIGHT_W;

  // --- Canvas render ----------------------------------------------------------
  useEffect(() => {
    const cvs = canvasRef.current;
    if (!cvs) return;
    const dpr = window.devicePixelRatio || 1;
    cvs.width = CANVAS_W * dpr;
    cvs.height = height * dpr;
    cvs.style.width = `${CANVAS_W}px`;
    cvs.style.height = `${height}px`;
    const ctx = cvs.getContext('2d')!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const bx = LEFT_W;
    const lx = bx + BAR_W;
    const rx = lx + TICK_W;

    // --- Gradient bar ---
    if (inverted) {
      ctx.save();
      ctx.scale(1, -1);
      ctx.drawImage(dataStrip, bx, -height, BAR_W, height);
      ctx.restore();
    } else {
      ctx.drawImage(dataStrip, bx, 0, BAR_W, height);
    }

    // Dim out-of-range regions
    ctx.fillStyle = 'rgba(0,0,0,0.55)';
    if (rangeMaxFrac < 1) {
      ctx.fillRect(bx, 0, BAR_W, (1 - rangeMaxFrac) * height);
    }
    if (rangeMinFrac > 0) {
      const dimY = (1 - rangeMinFrac) * height;
      ctx.fillRect(bx, dimY, BAR_W, height - dimY);
    }

    // Bar border
    ctx.strokeStyle = 'rgba(255,255,255,0.12)';
    ctx.lineWidth = 1;
    ctx.strokeRect(bx + 0.5, 0.5, BAR_W - 1, height - 1);

    // --- Data range edge labels (left of bar) ---
    ctx.fillStyle = 'rgb(220,220,220)';
    ctx.font = 'bold 12px "JetBrains Mono", "Fira Code", monospace';
    ctx.textBaseline = 'middle';
    ctx.textAlign = 'right';
    ctx.fillText(fmt(dataMax), bx - 5, 12);
    ctx.fillText(fmt(dataMin), bx - 5, height - 12);

    // --- Tick lines + labels ---
    ctx.textAlign = 'left';
    ctx.textBaseline = 'middle';
    ctx.fillStyle = 'rgba(255,255,255,0.55)';
    ctx.font = '11px "JetBrains Mono", "Fira Code", monospace';
    for (const t of ticks) {
      const y = (1 - t.y) * height;
      // tick line through bar
      ctx.strokeStyle = 'rgba(255,255,255,0.12)';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(bx, y);
      ctx.lineTo(bx + BAR_W, y);
      ctx.stroke();
      // label
      ctx.fillText(t.label, lx + 2, y);
    }

    // --- Handles (drawn first so labels go on top) ---
    const drawHandle = (y: number, color: string) => {
      ctx.strokeStyle = color;
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(bx, y);
      ctx.lineTo(bx + BAR_W, y);
      ctx.stroke();
      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.arc(bx + BAR_W / 2, y, 5, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = 'rgba(0,0,0,0.8)';
      ctx.lineWidth = 2;
      ctx.stroke();
    };

    const maxY = (1 - rangeMaxFrac) * height;
    const minY = (1 - rangeMinFrac) * height;
    drawHandle(maxY, '#ef4444');
    drawHandle(minY, '#3b82f6');

    // --- Handle value labels (on top of handles) ---
    // Max
    ctx.textAlign = 'left';
    ctx.textBaseline = 'top';
    ctx.fillStyle = '#ef4444';
    ctx.font = 'bold 14px "JetBrains Mono", "Fira Code", monospace';
    const maxLabel = fmt(rangeMax);
    const maxTextY = Math.max(10, maxY - 16);
    ctx.fillText(maxLabel, rx, maxTextY);

    // Min
    ctx.textBaseline = 'bottom';
    ctx.fillStyle = '#3b82f6';
    const minLabel = fmt(rangeMin);
    const minTextY = Math.min(height - 10, minY + 16);
    ctx.fillText(minLabel, rx, minTextY);

    // Unit label
    ctx.fillStyle = 'rgba(255,255,255,0.45)';
    ctx.font = '12px "JetBrains Mono", "Fira Code", monospace';
    ctx.textBaseline = 'middle';
    ctx.fillText(`°${tempUnit}`, rx + RIGHT_W - 20, height / 2);

  }, [height, rangeMinFrac, rangeMaxFrac, ticks, dataStrip, fmt, rangeMin, rangeMax, tempUnit, inverted]);

  // --- Pointer events ---------------------------------------------------------
  const handlePointerDown = (e: React.PointerEvent) => {
    const cvs = canvasRef.current;
    if (!cvs) return;
    cvs.setPointerCapture(e.pointerId);
    const rect = cvs.getBoundingClientRect();
    const frac = 1 - (e.clientY - rect.top) / rect.height;
    setDragging(Math.abs(frac - rangeMinFrac) <= Math.abs(frac - rangeMaxFrac) ? 'min' : 'max');
  };

  const handlePointerMove = (e: React.PointerEvent) => {
    if (!dragging || !canvasRef.current) return;
    const rect = canvasRef.current.getBoundingClientRect();
    const frac = 1 - Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
    const v = fracToTemp(frac);
    if (dragging === 'min') onMinChange(Math.min(v, rangeMax - 0.1));
    else onMaxChange(Math.max(v, rangeMin + 0.1));
  };

  const handlePointerUp = () => setDragging(null);

  return (
    <canvas
      ref={canvasRef}
      style={{ cursor: dragging ? 'ns-resize' : 'pointer', flexShrink: 0 }}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerLeave={handlePointerUp}
    />
  );
}
