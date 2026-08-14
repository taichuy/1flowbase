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
      assert.match(module.content_sha256, /^[a-f0-9]{64}$/);
      for (const asset of module.assets) {
        assert.match(asset.sha256, /^[a-f0-9]{64}$/);
        assert.ok(asset.bytes > 0);
      }
    }

    const richText = left.modules.find(
      (module) => module.module_source === '@1flowbase/rich-text'
    );
    assert.ok(richText, 'AC-REG-002 publishes the official rich-text module');
    const richTextBrowserAsset = richText.assets.find(
      (asset) => asset.role === 'browser_module'
    );
    assert.ok(
      richTextBrowserAsset,
      'AC-REG-002 publishes rich-text as a browser module'
    );
    const richTextSource = await readFile(
      join(first, richTextBrowserAsset.path),
      'utf8'
    );
    assert.doesNotMatch(
      richTextSource,
      /typeof require|require\.apply|typeof define/u,
      'AC-REG-002 keeps bare CommonJS and AMD probes out of the ESM catalog asset'
    );

    const charts = left.modules.find(
      (module) => module.module_source === '@1flowbase/charts'
    );
    assert.ok(charts, 'AC-004 publishes the official charts module');
    const chartsBrowserAsset = charts.assets.find(
      (asset) => asset.role === 'browser_module'
    );
    assert.ok(chartsBrowserAsset, 'AC-004 publishes charts as a browser module');
    const chartsSource = await readFile(
      join(first, chartsBrowserAsset.path),
      'utf8'
    );
    assert.match(
      chartsSource,
      /import \{ useEffect, useRef \} from "react";/u,
      'AC-004 preserves collision-safe React host import bindings'
    );
    assert.match(
      chartsSource,
      /import \{ jsx \} from "react\/jsx-runtime";/u,
      'AC-004 preserves the JSX runtime host import binding'
    );

    const icons = left.modules.find(
      (module) => module.module_source === '@ant-design/icons'
    );
    assert.ok(icons, 'AC-001 publishes the official Ant Design icons module');
    const iconsDescriptor = JSON.parse(
      await readFile(
        new URL(
          '../../packages/ant-design-icons-catalog/catalog-module.json',
          import.meta.url
        ),
        'utf8'
      )
    );
    const iconsPackage = JSON.parse(
      await readFile(
        new URL(
          '../../packages/ant-design-icons-catalog/package.json',
          import.meta.url
        ),
        'utf8'
      )
    );
    const hostIconsPackage = JSON.parse(
      await readFile(
        new URL(
          '../../app/node_modules/@ant-design/icons/package.json',
          import.meta.url
        ),
        'utf8'
      )
    );
    assert.equal(
      Object.hasOwn(iconsDescriptor, 'module_version'),
      false,
      'AC-001 keeps generated module versions out of the static Catalog descriptor'
    );
    assert.equal(
      iconsPackage.dependencies?.['@ant-design/icons'],
      undefined,
      'AC-001 keeps the host-owned dependency out of the Catalog package'
    );
    assert.equal(
      icons.module_version,
      hostIconsPackage.version,
      'AC-001 derives the Catalog module version from the host-resolved dependency'
    );

    const tailwind = left.modules.find(
      (module) => module.module_source === 'tailwindcss'
    );
    assert.ok(tailwind, 'AC-001 publishes the official tailwindcss module');
    assert.deepEqual(tailwind.compiler_identity, {
      name: '@1flowbase/tailwindcss-catalog',
      contract: 'block-preset-v1',
      tailwind_version: '4.3.3'
    });
    assert.deepEqual(tailwind.toolchain_lock, {
      package: 'tailwindcss',
      version: '4.3.3',
      mode: 'block-preset'
    });
    assert.doesNotMatch(
      tailwind.type_declarations,
      /TailwindCompiler|export function compile/u,
      'AC-001 exposes only the runtime metadata contract from the tailwindcss module'
    );
    assert.deepEqual(
      tailwind.assets.map((asset) => asset.role),
      ['shadow_style', 'browser_module'],
      'AC-001 publishes one source-independent ShadowRoot preset'
    );
    const presetAsset = tailwind.assets.find(
      (asset) => asset.role === 'shadow_style'
    );
    assert.ok(presetAsset, 'AC-001 publishes the Tailwind block preset');
    assert.equal(presetAsset.path, 'tailwindcss-catalog.css');
    assert.equal(
      presetAsset.sha256,
      '77c009cb4826b765d416513e3d9c83093482ecb69de9e361e4c25f5441240b36'
    );
    const presetCss = await readFile(join(first, presetAsset.path), 'utf8');
    assert.match(presetCss, /\.grid\{/u);
    assert.match(presetCss, /\.hover\\:bg-red-500:hover/u);
    assert.match(presetCss, /\.focus-visible\\:ring-2:focus-visible/u);
    assert.match(presetCss, /\.disabled\\:opacity-50:disabled/u);
    assert.match(presetCss, /\.md\\:grid-cols-2/u);
    assert.match(presetCss, /::file-selector-button/u);
    assert.doesNotMatch(presetCss, /\.ant-/u);
    const tailwindBrowserAsset = tailwind.assets.find(
      (asset) => asset.role === 'browser_module'
    );
    assert.ok(
      tailwindBrowserAsset,
      'AC-001 publishes the Tailwind runtime metadata module'
    );
    const tailwindBrowserSource = await readFile(
      join(first, tailwindBrowserAsset.path),
      'utf8'
    );
    assert.ok(
      Buffer.byteLength(tailwindBrowserSource) < 4096,
      'AC-001 keeps the executable compiler out of the runtime metadata module'
    );
    assert.doesNotMatch(
      tailwindBrowserSource,
      /\brequire\b|compileTailwindExecutableArtifact|sucrase/u,
      'AC-001 keeps compiler dependency syntax out of the runtime module registry'
    );
    const [legacyTailwind] = left.retained_legacy_assets;
    assert.deepEqual(legacyTailwind, {
      identity: 'tailwindcss-inventory-v1',
      path: 'tailwindcss-inventory-v1.css',
      media_type: 'text/css; charset=utf-8',
      sha256:
        '14b8d5ee303508395223aa26fff3de63c24dbc01a0d0a425cb822e91dd517c9c',
      bytes: 29614,
      use: 'legacy-recognition-only'
    });
    const css = await readFile(join(first, legacyTailwind.path), 'utf8');
    assert.match(css, /\.grid\{/u);
    assert.match(css, /\.gap-4\{/u);
    assert.match(css, /\.p-4\{/u);
    assert.doesNotMatch(css, /@layer base/u);
    assert.doesNotMatch(css, /(?:^|\})\s*(?:\*|button|input|h[1-6])(?:,|\{)/u);
    assert.doesNotMatch(css, /\.ant-/u);
  } finally {
    await Promise.all([
      rm(first, { force: true, recursive: true }),
      rm(second, { force: true, recursive: true })
    ]);
  }
});
