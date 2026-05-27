import { useState, useRef, useCallback, useMemo, useEffect } from 'react';
import { parseThermalImage } from '@/lib/format-detector';
import { createTempCanvas } from '@/lib/irg-parser';
import type { RenderOpts } from '@/lib/irg-parser';
import { CURSOR_COLORS, PALETTES, OVERSCAN_OPTIONS } from '@/lib/constants';
import { toUnit } from '@/lib/units';
import type { MeasurementCursor, OverlayConfig, FileInfo, TempUnit, Overscan, ScaleMode, Palette, ThermalImage } from '@/lib/types';
import { ThermalCanvas } from '@/components/ThermalCanvas';
import { RangeColorBar } from '@/components/RangeColorBar';
import { CursorPanel } from '@/components/CursorPanel';

function StatPill({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex flex-col px-4 py-2.5 bg-thermal-surface/60">
      <span className="text-[0.6rem] tracking-[0.14em] uppercase text-thermal-muted font-display">{label}</span>
      <span className="font-display text-sm text-thermal-heading tabular-nums mt-0.5">{value}</span>
    </div>
  );
}

export function ThermalViewer() {
  const [thermalImage, setThermalImage] = useState<ThermalImage | null>(null);
  const [palette, setPalette] = useState<Palette>('inferno');
  const [tempUnit, setTempUnit] = useState<TempUnit>('C');
  const [scaleMode, setScaleMode] = useState<ScaleMode>('linear');
  const [rangeMin, setRangeMin] = useState(0);
  const [rangeMax, setRangeMax] = useState(0);
  const [overscan, setOverscan] = useState<Overscan>('clip');
  const [scale, setScale] = useState(3);
  const [inverted, setInverted] = useState(false);
  const [labelScale, setLabelScale] = useState(10);
  const [hoverTemp, setHoverTemp] = useState<number | null>(null);
  const [cursors, setCursors] = useState<MeasurementCursor[]>([]);
  const [overlay, setOverlay] = useState<OverlayConfig>({ showMinMaxSpots: true, showEmissivity: true, showTimestamp: true });
  const [fileInfo, setFileInfo] = useState<FileInfo>({ name: '', modified: null });
  const cursorIdRef = useRef(0);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const exportCanvasRef = useRef<HTMLCanvasElement | null>(null);

  const renderOpts: RenderOpts = useMemo(() => ({
    palette, minC: rangeMin, maxC: rangeMax, overscan, scaleMode,
    belowColor: [0, 0, 128] as [number, number, number],
    aboveColor: [255, 255, 255] as [number, number, number],
    inverted,
    cdfLut: thermalImage?.cdfLut ?? undefined,
  }), [palette, rangeMin, rangeMax, overscan, scaleMode, inverted, thermalImage?.cdfLut]);

  const renderedCanvas = useMemo(() => {
    if (!thermalImage) return null;
    return createTempCanvas(thermalImage.celsius, thermalImage.width, thermalImage.height, renderOpts);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [thermalImage?.celsius, renderOpts]); // implicitly depends on thermalImage dimensions

  const processFile = useCallback((buf: ArrayBuffer, fileName?: string, modified?: number | null) => {
    try {
      const img = parseThermalImage(buf, fileName, modified);
      setThermalImage(img);
      setRangeMin(img.dataMin);
      setRangeMax(img.dataMax);
      setFileInfo({ name: img.fileName, modified: img.fileModified });
    } catch (err) { alert('Failed to parse thermal image: ' + (err as Error).message); }
  }, []);

  const upload = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const f = e.target.files?.[0]; if (!f) return;
    const r = new FileReader();
    r.onload = ev => processFile(ev.target!.result as ArrayBuffer, f.name, f.lastModified);
    r.readAsArrayBuffer(f);
  }, [processFile]);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    const f = e.dataTransfer.files?.[0]; if (!f) return;
    const r = new FileReader();
    r.onload = ev => processFile(ev.target!.result as ArrayBuffer, f.name, f.lastModified);
    r.readAsArrayBuffer(f);
  }, [processFile]);

  const addCursor = useCallback((x: number, y: number, tempC: number) => {
    const id = ++cursorIdRef.current;
    const colorIdx = cursors.length % CURSOR_COLORS.length;
    setCursors(prev => [...prev, { id, x, y, label: `${id}`, colorIdx, tempC }]);
  }, [cursors.length]);

  const removeCursor = useCallback((id: number) => setCursors(prev => prev.filter(c => c.id !== id)), []);
  const renameCursor = useCallback((id: number, label: string) =>
    setCursors(prev => prev.map(c => c.id === id ? { ...c, label } : c)), []);

  const clear = useCallback(() => {
    setThermalImage(null);
    setCursors([]);
    setFileInfo({ name: '', modified: null });
    setOverlay({ showMinMaxSpots: true, showEmissivity: true, showTimestamp: true });
    if (fileInputRef.current) fileInputRef.current.value = '';
  }, []);

  const autoRange = () => {
    if (!thermalImage) return;
    setRangeMin(thermalImage.dataMin);
    setRangeMax(thermalImage.dataMax);
  };
  const range20_40 = () => { setRangeMin(20); setRangeMax(40); };
  const range0_80 = () => { setRangeMin(0); setRangeMax(80); };
  const rangeNeg10_50 = () => { setRangeMin(-10); setRangeMax(50); };

  // Fit image within viewport height. Width overflow is handled by CSS max-w-full.
  const [maxImgH, setMaxImgH] = useState(600);
  useEffect(() => {
    const calc = () => {
      setMaxImgH(window.innerHeight - 220);
    };
    calc();
    window.addEventListener('resize', calc);
    return () => window.removeEventListener('resize', calc);
  }, []);
  const effectiveScale = thermalImage
    ? Math.min(scale, Math.floor(maxImgH / thermalImage.height))
    : 1;
  const displayH = thermalImage ? thermalImage.height * effectiveScale : 0;

  const download = useCallback(() => {
    const cvs = exportCanvasRef.current; if (!cvs) return;
    const a = document.createElement('a');
    a.download = 'thermal-image.png';
    a.href = cvs.toDataURL('image/png');
    a.click();
  }, []);

  return (
    <div className="max-w-[1200px] mx-auto py-6 px-4 space-y-6">
      <header className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="size-2.5 rounded-full bg-thermal-hot shadow-[0_0_10px] shadow-thermal-hot/50 animate-pulse" />
          <h1 className="font-display text-base tracking-tight text-thermal-heading">THERMVIEW</h1>
        </div>
        <span className="font-display text-[0.65rem] text-thermal-muted tracking-[0.2em]">MULTI-FORMAT THERMAL ANALYZER</span>
      </header>

      {!thermalImage ? (
        <div onDragOver={e => e.preventDefault()} onDrop={handleDrop} onClick={() => fileInputRef.current?.click()}
          className="relative group cursor-pointer border-2 border-dashed border-thermal-border rounded-2xl p-20 flex flex-col items-center gap-4 hover:border-thermal-accent/40 transition-colors duration-300">
          <input ref={fileInputRef} type="file" accept=".irg,.jpg,.jpeg,.img,.seq" onChange={upload} className="hidden" />
          <div className="size-16 rounded-2xl bg-thermal-surface flex items-center justify-center group-hover:bg-thermal-accent/10 transition-colors">
            <svg className="size-7 text-thermal-muted group-hover:text-thermal-accent transition-colors" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5m-13.5-9L12 3m0 0 4.5 4.5M12 3v13.5" />
            </svg>
          </div>
          <p className="font-display text-sm text-thermal-heading">Drop thermal image here</p>
          <p className="text-xs text-thermal-muted mt-1">Supports IRG, Hikmicro, DJI — drag or click to browse</p>
        </div>
      ) : (
        <>
          <div className="space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <div className="flex flex-wrap gap-1 bg-thermal-surface rounded-lg p-1">
                {PALETTES.map(p => (
                  <button key={p.value} onClick={() => setPalette(p.value)}
                    className={`px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold tracking-wide transition-all ${palette === p.value ? 'bg-thermal-accent text-black shadow-[0_1px_6px] shadow-thermal-accent/30' : 'text-thermal-muted hover:text-thermal-text'}`}>
                    {p.label}
                  </button>
                ))}
              </div>
              <div className="w-px h-6 bg-thermal-border" />
              <span className="font-display text-[0.55rem] text-thermal-muted uppercase tracking-wider">Unit</span>
              <div className="flex rounded-lg bg-thermal-surface p-0.5">
                {(['C','F'] as TempUnit[]).map(u => (
                  <button key={u} onClick={() => setTempUnit(u)}
                    className={`px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold transition-all ${tempUnit === u ? 'bg-thermal-accent text-black shadow-[0_1px_6px] shadow-thermal-accent/30' : 'text-thermal-muted hover:text-thermal-text'}`}>
                    °{u}
                  </button>
                ))}
              </div>
              <div className="flex-1" />
              <span className="font-display text-[0.55rem] text-thermal-muted uppercase tracking-wider">Scale</span>
              <div className="flex items-center gap-1 bg-thermal-surface rounded-lg p-0.5">
                {[1,2,3,4,5,6].map(s => (
                  <button key={s} onClick={() => setScale(s)}
                    className={`px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold transition-all ${scale === s ? 'bg-thermal-accent text-black' : 'text-thermal-muted hover:text-thermal-text'}`}>
                    {s}×
                  </button>
                ))}
              </div>
              <div className="flex rounded-lg bg-black/20 p-0.5">
                <button onClick={download}
                  className="px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold transition-all text-thermal-heading hover:text-thermal-text hover:bg-white/5">
                  Export
                </button>
                <button onClick={clear}
                  className="px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold transition-all text-thermal-muted hover:text-thermal-text hover:bg-white/5">
                  Clear
                </button>
              </div>
            </div>

            <div className="flex flex-wrap items-center gap-2 bg-thermal-surface rounded-lg px-3 py-2">
              <span className="font-display text-[0.65rem] text-thermal-muted uppercase tracking-wider">Range</span>
              <div className="flex rounded-lg bg-black/20 p-0.5">
                <button onClick={autoRange}
                  className="px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold transition-all text-thermal-accent hover:text-thermal-heading hover:bg-white/5">
                  Auto
                </button>
                <button onClick={range20_40}
                  className="px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold transition-all text-thermal-muted hover:text-thermal-text hover:bg-white/5">
                  20–40°{tempUnit}
                </button>
                <button onClick={range0_80}
                  className="px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold transition-all text-thermal-muted hover:text-thermal-text hover:bg-white/5">
                  0–80°{tempUnit}
                </button>
                <button onClick={rangeNeg10_50}
                  className="px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold transition-all text-thermal-muted hover:text-thermal-text hover:bg-white/5">
                  −10–50°{tempUnit}
                </button>
              </div>
              <div className="w-px h-5 bg-thermal-border" />
              <span className="font-display text-[0.55rem] text-thermal-muted uppercase tracking-wider">Overscan</span>
              <div className="flex rounded-lg bg-black/20 p-0.5">
                {OVERSCAN_OPTIONS.map(o => (
                  <button key={o.value} onClick={() => setOverscan(o.value)}
                    title={o.value === 'clip' ? 'Clamp to palette edge' : o.value === 'none' ? 'Hide out-of-range' : o.value === 'below' ? 'Below-range as blue' : 'Above-range as white'}
                    className={`px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold transition-all ${overscan === o.value ? 'bg-thermal-accent text-black shadow-[0_1px_6px] shadow-thermal-accent/30' : 'text-thermal-muted hover:text-thermal-text'}`}>
                    {o.label}
                  </button>
                ))}
              </div>
              <div className="w-px h-5 bg-thermal-border" />
              <span className="font-display text-[0.55rem] text-thermal-muted uppercase tracking-wider">Mapping</span>
              <div className="flex rounded-lg bg-black/20 p-0.5">
                {(['linear','log','equalize'] as ScaleMode[]).map(m => (
                  <button key={m} onClick={() => setScaleMode(m)}
                    title={m === 'linear' ? 'Uniform color spread' : m === 'log' ? 'More detail at cold end' : 'Percentile stretch (full palette)'}
                    className={`px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold transition-all capitalize ${scaleMode === m ? 'bg-thermal-accent text-black shadow-[0_1px_6px] shadow-thermal-accent/30' : 'text-thermal-muted hover:text-thermal-text'}`}>
                    {m === 'equalize' ? 'Eq' : m}
                  </button>
                ))}
              </div>
              <div className="w-px h-5 bg-thermal-border" />
              <span className="font-display text-[0.55rem] text-thermal-muted uppercase tracking-wider">Invert</span>
              <button onClick={() => setInverted(i => !i)}
                className={`px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold transition-all ${inverted ? 'bg-thermal-accent text-black shadow-[0_1px_6px] shadow-thermal-accent/30' : 'text-thermal-muted hover:text-thermal-text'}`}>Invert</button>
              <div className="w-px h-5 bg-thermal-border" />
              <span className="font-display text-[0.55rem] text-thermal-muted uppercase tracking-wider">Overlay</span>
              <div className="flex rounded-lg bg-black/20 p-0.5">
                <button onClick={() => setOverlay(o => ({ ...o, showMinMaxSpots: !o.showMinMaxSpots }))}
                  title="Show min/max temperature markers"
                  className={`px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold transition-all ${overlay.showMinMaxSpots ? 'bg-thermal-accent text-black shadow-[0_1px_6px] shadow-thermal-accent/30' : 'text-thermal-muted hover:text-thermal-text'}`}>
                  MinMax
                </button>
                <button onClick={() => setOverlay(o => ({ ...o, showEmissivity: !o.showEmissivity }))}
                  title="Show emissivity & file info corner pills"
                  className={`px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold transition-all ${overlay.showEmissivity ? 'bg-thermal-accent text-black shadow-[0_1px_6px] shadow-thermal-accent/30' : 'text-thermal-muted hover:text-thermal-text'}`}>
                  Info
                </button>
                <button onClick={() => setOverlay(o => ({ ...o, showTimestamp: !o.showTimestamp }))}
                  title="Show file timestamp corner pill"
                  className={`px-2.5 py-1.5 rounded-md font-display text-[0.7rem] font-semibold transition-all ${overlay.showTimestamp ? 'bg-thermal-accent text-black shadow-[0_1px_6px] shadow-thermal-accent/30' : 'text-thermal-muted hover:text-thermal-text'}`}>
                  Time
                </button>
              </div>
              <div className="w-px h-5 bg-thermal-border" />
              <span className="font-display text-[0.65rem] text-thermal-muted">
                Window: <span className="text-thermal-cold">{toUnit(rangeMin, tempUnit)}</span>
                <span className="mx-1">–</span>
                <span className="text-thermal-hot">{toUnit(rangeMax, tempUnit)}</span>
                <span className="ml-1">°{tempUnit}</span>
              </span>
              <span className="ml-auto text-[0.6rem] text-thermal-muted font-display">
                File: {toUnit(thermalImage.dataMin, tempUnit)}–{toUnit(thermalImage.dataMax, tempUnit)}°{tempUnit}
              </span>
            </div>
          </div>

          {thermalImage && renderedCanvas && (
            <div className="flex items-stretch gap-3">
              <RangeColorBar palette={palette} scaleMode={scaleMode}
                dataMin={thermalImage.dataMin} dataMax={thermalImage.dataMax}
                rangeMin={rangeMin} rangeMax={rangeMax} tempUnit={tempUnit}
                height={displayH || 300}
                onMinChange={setRangeMin} onMaxChange={setRangeMax} />
              <div className="relative flex-1 min-w-0 overflow-hidden">
                <ThermalCanvas
                  image={thermalImage}
                  renderOpts={renderOpts} cursors={cursors}
                  scale={effectiveScale} tempUnit={tempUnit}
                  labelScale={labelScale}
                  overlay={overlay}
                  fileInfo={fileInfo}
                  onCursorAdd={addCursor}
                  onHover={setHoverTemp}
                  exportRef={exportCanvasRef}
                />
                {hoverTemp !== null && (
                  <div className="absolute top-2 right-2 pointer-events-none bg-black/80 backdrop-blur-sm border border-white/10 rounded px-2 py-1 font-display text-xs text-white z-20">
                    {toUnit(hoverTemp, tempUnit)}°{tempUnit}
                  </div>
                )}
              </div>
              <CursorPanel cursors={cursors} tempUnit={tempUnit} labelScale={labelScale}
                onRename={renameCursor} onDelete={removeCursor} onLabelScaleChange={setLabelScale} />
            </div>
          )}

          <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-px rounded-lg overflow-hidden border border-thermal-border bg-thermal-border">
            <StatPill label="Resolution" value={`${thermalImage.width}×${thermalImage.height}`} />
            <StatPill label="Emissivity" value={thermalImage.emissivity.toFixed(3)} />
            <StatPill label="Air Temp" value={`${toUnit(thermalImage.airTemp, tempUnit)}°${tempUnit}`} />
            <StatPill label="Ref Temp" value={`${toUnit(thermalImage.refTemp, tempUnit)}°${tempUnit}`} />
            <StatPill label="Distance" value={`${thermalImage.distance.toFixed(1)}m`} />
            <StatPill label="Atm Trans" value={thermalImage.atmTrans.toFixed(3)} />
          </div>
        </>
      )}
    </div>
  );
}
