import { gunzipSync } from 'node:zlib';
import { expect, test } from 'vitest';
import { staticAssetCompressionPlugin } from '../compression';

test('text sidecars preserve exact bytes, are deterministic, and leave binary assets alone', async () => {
  const plugin = staticAssetCompressionPlugin();
  const hook = plugin.generateBundle;
  if (typeof hook !== 'object') throw new Error('Expected final ordered hook');
  expect(hook.order).toBe('post');
  const files = {
    'entry.js': {
      type: 'chunk',
      fileName: 'entry.js',
      code: 'export const text = "完整类型";'
    },
    'types.json': {
      type: 'asset',
      fileName: 'types.json',
      source: JSON.stringify([{ content: 'declare type T = string;' }])
    },
    'style.css': {
      type: 'asset',
      fileName: 'style.css',
      source: 'body { color: red }'
    },
    'module.wasm': {
      type: 'asset',
      fileName: 'module.wasm',
      source: new Uint8Array([0, 1, 2])
    }
  };
  const run = async () => {
    const emitted: { fileName: string; source: Uint8Array }[] = [];
    await (hook.handler as Function).call(
      { emitFile: (file: (typeof emitted)[number]) => emitted.push(file) },
      {},
      files
    );
    return emitted;
  };
  const first = await run();
  const second = await run();
  expect(first.map((f) => f.fileName)).toEqual([
    'entry.js.gz',
    'types.json.gz',
    'style.css.gz'
  ]);
  for (let i = 0; i < first.length; i++) {
    expect(first[i]!.source).toEqual(second[i]!.source);
    const original =
      files[first[i]!.fileName.slice(0, -3) as keyof typeof files];
    expect(gunzipSync(first[i]!.source).toString()).toBe(
      'code' in original ? original.code : original.source
    );
  }
});
