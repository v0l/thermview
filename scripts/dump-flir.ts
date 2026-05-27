import { readFileSync } from 'node:fs';
import { parseFLIRRJPEG } from '../src/lib/flir-rjpeg-parser.ts';

const file = process.argv[2] || 'test-samples/flir_rjpeg_example.jpg';
const buf = readFileSync(file).buffer as ArrayBuffer;
const img = parseFLIRRJPEG(buf, file, null);

const { width, height, celsius, rawValues, dataMin, dataMax } = img;

console.log(`File: ${file}`);
console.log(`Size: ${width}×${height}`);
console.log(`Data min: ${dataMin.toFixed(2)}°C  max: ${dataMax.toFixed(2)}°C`);
console.log();

// Column at middle-x
const x = Math.floor(width / 2);
console.log(`Middle column (x=${x}) top-to-bottom, 40 samples:`);
const step = Math.max(1, Math.floor(height / 40));
for (let i = 0; i < height; i += step) {
  const idx = i * width + x;
  console.log(
    `  y=${String(i).padStart(4)}  raw=${String(rawValues[idx]).padStart(6)}  °C=${celsius[idx].toFixed(2)}`,
  );
}

console.log();

// Histogram
const NUM_BUCKETS = 30;
const buckets = new Int32Array(NUM_BUCKETS);
let below = 0, above = 0;
for (let i = 0; i < celsius.length; i++) {
  const c = celsius[i];
  if (c < dataMin) { below++; continue; }
  if (c > dataMax) { above++; continue; }
  const bucket = Math.floor((c - dataMin) / (dataMax - dataMin) * (NUM_BUCKETS - 1));
  buckets[Math.min(bucket, NUM_BUCKETS - 1)]++;
}

console.log('Temperature histogram:');
for (let i = 0; i < NUM_BUCKETS; i++) {
  const temp = dataMin + (i + 0.5) * (dataMax - dataMin) / NUM_BUCKETS;
  const bar = '█'.repeat(Math.round(buckets[i] / (width * height / 500)));
  console.log(`  ${temp.toFixed(1).padStart(6)}°C: ${String(buckets[i]).padStart(6)} ${bar}`);
}