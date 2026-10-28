import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { brotliDecompressSync, gunzipSync } from 'node:zlib';
import { expect, test } from 'vitest';
import { createAssetReader, preferredEncoding } from '../vditor-compression';

test('negotiates supported encodings and respects explicit exclusions', () => {
  expect(preferredEncoding('gzip, br')).toBe('br');
  expect(preferredEncoding('br;q=0, gzip;q=0.8')).toBe('gzip');
  expect(preferredEncoding('br;q=0.2, gzip;q=0.9')).toBe('gzip');
  expect(preferredEncoding('br;q=0, gzip;q=0, *;q=1')).toBeUndefined();
  expect(preferredEncoding()).toBeUndefined();
});

test('compression preserves the actual parser bytes and reuses cached work', async () => {
  const require = createRequire(import.meta.url);
  const file = path.join(
    path.dirname(require.resolve('vditor/package.json')),
    'dist/js/lute/lute.min.js'
  );
  const read = createAssetReader();
  const source = await readFile(file);
  const pending = read(file, 'br');
  expect(read(file, 'br')).toBe(pending);
  const compressed = await pending;
  expect(compressed.length).toBeLessThan(source.length / 5);
  expect(brotliDecompressSync(compressed).equals(source)).toBe(true);
  expect(gunzipSync(await read(file, 'gzip')).equals(source)).toBe(true);
  expect((await read(file)).equals(source)).toBe(true);
});
