import { CURSOR_COLORS } from '@/lib/constants';

/** Quick rounded-rect path helper. */
function roundedRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + w - r, y); ctx.arcTo(x + w, y, x + w, y + r, r);
  ctx.lineTo(x + w, y + h - r); ctx.arcTo(x + w, y + h, x + w - r, y + h, r);
  ctx.lineTo(x + r, y + h); ctx.arcTo(x, y + h, x, y + h - r, r);
  ctx.lineTo(x, y + r); ctx.arcTo(x, y, x + r, y, r);
  ctx.closePath();
}

export function drawCrosshair(
  ctx: CanvasRenderingContext2D,
  cx: number, cy: number,
  color: typeof CURSOR_COLORS[number],
  fontSize: number, factor: number,
) {
  // Compute lowest point of crosshair to position label below it
  const f = Math.max(1, Math.round(fontSize * 0.25));
  const ringR = f + factor * 1.5;
  const armLen = ringR + f;
  const gap = f * 0.5 + factor * 0.5;

  ctx.strokeStyle = color.stroke;
  ctx.lineWidth = Math.max(1, Math.round(fontSize * 0.08));
  ctx.beginPath();
  ctx.arc(cx, cy, ringR, 0, Math.PI * 2);
  ctx.moveTo(cx - armLen, cy); ctx.lineTo(cx - gap, cy);
  ctx.moveTo(cx + gap, cy); ctx.lineTo(cx + armLen, cy);
  ctx.moveTo(cx, cy - armLen); ctx.lineTo(cx, cy - gap);
  ctx.moveTo(cx, cy + gap); ctx.lineTo(cx, cy + armLen);
  ctx.stroke();

  ctx.fillStyle = color.fill;
  ctx.beginPath(); ctx.arc(cx, cy, f * 0.35, 0, Math.PI * 2); ctx.fill();

  return armLen; // used by drawLabel for spacing
}

export function drawLabel(
  ctx: CanvasRenderingContext2D,
  cx: number, cy: number,
  text: string,
  color: typeof CURSOR_COLORS[number],
  fontSize: number,
  crosshairArmLen: number,
  dispW: number,
) {
  ctx.font = `600 ${fontSize}px "JetBrains Mono", "Consolas", monospace`;
  const tm = ctx.measureText(text);
  const pw = Math.max(2, Math.round(fontSize * 0.3));
  const ph = Math.max(1, Math.round(fontSize * 0.12));
  const lw = tm.width + pw * 2;
  const lh = fontSize + ph * 2;
  const ly = cy + crosshairArmLen + fontSize * 0.6;
  let lx = cx - lw / 2;
  lx = Math.max(0, Math.min(lx, dispW - lw));
  ctx.fillStyle = 'rgba(0,0,0,0.85)';
  roundedRect(ctx, lx, ly, lw, lh, 2);
  ctx.fill();
  ctx.strokeStyle = color.stroke; ctx.lineWidth = 1; ctx.stroke();
  ctx.fillStyle = color.fill; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
  ctx.fillText(text, lx + lw / 2, ly + lh / 2);
}

// ─── Min / Max marker (triangle icon + temp readout) ────────────────────────

export function drawMinMaxMarker(
  ctx: CanvasRenderingContext2D,
  cx: number, cy: number,
  tempC: number,
  kind: 'min' | 'max',
  tempUnit: 'C' | 'F',
  fontSize: number,
  dispW: number,
) {
  const color = kind === 'max' ? '#ef4444' : '#3b82f6';
  const label = kind === 'max' ? 'MAX' : 'MIN';
  const tempVal = tempUnit === 'F' ? ((tempC * 9) / 5 + 32).toFixed(1) : tempC.toFixed(1);
  const text = `${label}: ${tempVal}°${tempUnit}`;
  const triSize = Math.max(4, Math.round(fontSize * 0.5));

  ctx.save();
  // Triangle icon
  ctx.fillStyle = color;
  ctx.beginPath();
  if (kind === 'max') {
    ctx.moveTo(cx, cy - triSize);
    ctx.lineTo(cx - triSize * 0.75, cy + triSize * 0.5);
    ctx.lineTo(cx + triSize * 0.75, cy + triSize * 0.5);
  } else {
    ctx.moveTo(cx, cy + triSize);
    ctx.lineTo(cx - triSize * 0.75, cy - triSize * 0.5);
    ctx.lineTo(cx + triSize * 0.75, cy - triSize * 0.5);
  }
  ctx.closePath();
  ctx.fill();
  ctx.strokeStyle = 'rgba(0,0,0,0.6)';
  ctx.lineWidth = 1;
  ctx.stroke();

  // Label
  const fs = Math.max(7, fontSize * 0.65);
  ctx.font = `600 ${fs}px "JetBrains Mono", "Consolas", monospace`;
  const metrics = ctx.measureText(text);
  const pw = 4;
  const ph = 2;
  const lw = metrics.width + pw * 2;
  const lh = fs + ph * 2;
  const offsetY = kind === 'max' ? -(triSize + lh + 2) : triSize + 4;
  let lx = cx - lw / 2;
  lx = Math.max(0, Math.min(lx, dispW - lw));
  const ly = cy + offsetY;

  ctx.fillStyle = 'rgba(0,0,0,0.85)';
  roundedRect(ctx, lx, ly, lw, lh, 2);
  ctx.fill();
  ctx.strokeStyle = color;
  ctx.lineWidth = 1;
  ctx.stroke();

  ctx.fillStyle = color;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(text, lx + lw / 2, ly + lh / 2);
  ctx.restore();
}

// ─── Corner info pill (emissivity, timestamp, etc.) ─────────────────────────

export function drawInfoPill(
  ctx: CanvasRenderingContext2D,
  text: string,
  corner: 'topLeft' | 'topRight' | 'bottomLeft' | 'bottomRight',
  dispW: number,
  dispH: number,
  fontSize: number,
) {
  const fs = Math.max(7, fontSize * 0.6);
  ctx.font = `600 ${fs}px "JetBrains Mono", "Consolas", monospace`;
  const metrics = ctx.measureText(text);
  const padX = 6;
  const padY = 3;
  const pw = metrics.width + padX * 2;
  const ph = fs + padY * 2;
  const margin = 6;

  let px: number, py: number;
  if (corner === 'topLeft')       { px = margin; py = margin; }
  else if (corner === 'topRight')  { px = dispW - pw - margin; py = margin; }
  else if (corner === 'bottomLeft'){ px = margin; py = dispH - ph - margin; }
  else                             { px = dispW - pw - margin; py = dispH - ph - margin; }

  ctx.save();
  ctx.fillStyle = 'rgba(0,0,0,0.75)';
  ctx.strokeStyle = 'rgba(255,255,255,0.12)';
  ctx.lineWidth = 1;
  roundedRect(ctx, px, py, pw, ph, 3);
  ctx.fill();
  ctx.stroke();

  ctx.fillStyle = 'rgba(255,255,255,0.85)';
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(text, px + pw / 2, py + ph / 2);
  ctx.restore();
}
