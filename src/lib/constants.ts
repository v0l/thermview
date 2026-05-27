import type { Palette, Overscan } from '@/lib/irg-parser';

export const CURSOR_COLORS = [
  { stroke: '#ef4444', fill: '#ef4444' },
  { stroke: '#22c55e', fill: '#22c55e' },
  { stroke: '#3b82f6', fill: '#3b82f6' },
  { stroke: '#eab308', fill: '#eab308' },
  { stroke: '#06b6d4', fill: '#06b6d4' },
  { stroke: '#d946ef', fill: '#d946ef' },
  { stroke: '#f97316', fill: '#f97316' },
  { stroke: '#84cc16', fill: '#84cc16' },
];

export const PALETTES: { value: Palette; label: string }[] = [
  { value: 'inferno', label: 'Inferno' },
  { value: 'iron', label: 'Iron' },
  { value: 'jet', label: 'Jet' },
  { value: 'hot', label: 'Hot' },
  { value: 'lava', label: 'Lava' },
  { value: 'plasma', label: 'Plasma' },
  { value: 'rainbow', label: 'Rainbow' },
  { value: 'arctic', label: 'Arctic' },
  { value: 'grayscale', label: 'Grayscale' },
];

export const OVERSCAN_OPTIONS: { value: Overscan; label: string }[] = [
  { value: 'clip', label: 'Clip' },
  { value: 'none', label: 'None' },
  { value: 'below', label: 'Below' },
  { value: 'above', label: 'Above' },
];
