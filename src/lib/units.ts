import type { TempUnit } from '@/lib/types';

export function toUnit(celsius: number, unit: TempUnit): string {
  if (unit === 'F') return ((celsius * 9) / 5 + 32).toFixed(1);
  return celsius.toFixed(1);
}
