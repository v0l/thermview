import { CURSOR_COLORS } from '@/lib/constants';
import { toUnit } from '@/lib/units';
import type { MeasurementCursor, TempUnit } from '@/lib/types';

export function CursorPanel({
  cursors, tempUnit, labelScale, onRename, onDelete, onLabelScaleChange,
}: {
  cursors: MeasurementCursor[];
  tempUnit: TempUnit;
  labelScale: number;
  onRename: (id: number, label: string) => void;
  onDelete: (id: number) => void;
  onLabelScaleChange: (v: number) => void;
}) {
  return (
    <div className="w-44 flex-shrink-0 flex flex-col gap-1.5">
      <div className="flex items-center gap-1.5 mb-0.5">
        <span className="font-display text-[0.55rem] text-thermal-muted uppercase tracking-wider">Labels</span>
        <input type="range" min={5} max={30} step={1} value={labelScale}
          onChange={e => onLabelScaleChange(parseFloat(e.target.value))} className="flex-1 accent-thermal-accent h-3" />
        <span className="text-[0.5rem] text-thermal-muted tabular-nums w-5 text-right">{labelScale}</span>
      </div>
      {cursors.length === 0 ? (
        <span className="text-[0.55rem] text-thermal-muted leading-relaxed">Click the image to add points</span>
      ) : (
        cursors.map(c => {
          const col = CURSOR_COLORS[c.colorIdx];
          return (
            <div key={c.id} className="flex items-center gap-1.5 bg-thermal-surface rounded-lg px-2 py-1.5 group">
              <div className="size-2 rounded-full flex-shrink-0" style={{ backgroundColor: col.fill }} />
              <input value={c.label} onChange={e => onRename(c.id, e.target.value)}
                className="flex-1 bg-transparent text-[0.65rem] font-mono text-thermal-heading outline-none min-w-0" />
              <span className="text-[0.55rem] text-thermal-muted tabular-nums whitespace-nowrap">{toUnit(c.tempC, tempUnit)}°</span>
              <button onClick={() => onDelete(c.id)}
                className="opacity-0 group-hover:opacity-60 hover:!opacity-100 text-[0.55rem] text-thermal-muted hover:text-red-400 transition-all ml-0.5">×</button>
            </div>
          );
        })
      )}
    </div>
  );
}
