import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import { buildOfficialBrowserAssets } from '../build-official-browser-assets.mjs';

test('AC-REG-001 emits stable exports, types, assets and digest inputs', async () => {
  const first = await mkdtemp(join(tmpdir(), '1flowbase-assets-a-'));
  const second = await mkdtemp(join(tmpdir(), '1flowbase-assets-b-'));
  try {
    const left = await buildOfficialBrowserAssets(first);
    const right = await buildOfficialBrowserAssets(second);
    assert.deepEqual(left, right);

    const expected = JSON.parse(
      await readFile(
        new URL(
          './fixtures/official-browser-assets.expected.json',
          import.meta.url
        ),
        'utf8'
      )
    );
    assert.equal(left.format, expected.format);
    assert.deepEqual(
      left.modules.map((module) => module.module_source),
      expected.module_sources
    );
    for (const module of left.modules) {
      assert.deepEqual(module.exports, expected.exports[module.module_source]);
      assert.match(module.type_declarations, /declare module/);
      for (const asset of module.assets)
        assert.match(asset.sha256, /^[a-f0-9]{64}$/);
    }
  } finally {
    await Promise.all([
      rm(first, { force: true, recursive: true }),
      rm(second, { force: true, recursive: true })
    ]);
  }
});
