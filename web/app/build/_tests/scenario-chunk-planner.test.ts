import { describe, expect, test } from 'vitest';

import { planScenarioChunk } from '../scenario-chunk-planner';
import { productAntDesignChunk } from '../native-antd-es-modules';

describe('scenario chunk planner', () => {
  test.each([
    ['/repo/node_modules/react/jsx-runtime.js', 'react-runtime'],
    ['/repo/node_modules/react-dom/client.js', 'react-runtime']
  ])('places %s in %s', (id, chunk) => {
    expect(planScenarioChunk(id)).toBe(chunk);
  });

  test.each([
    ['antd/es/button', 'antd-core'],
    ['antd/es/modal', 'antd-overlay'],
    ['antd/es/table', 'antd-data'],
    ['antd/es/result', 'antd-feedback']
  ])('classifies product source %s as %s', (source, chunk) => {
    expect(productAntDesignChunk(source)).toBe(chunk);
  });

  test('keeps application modules under route and demand-island ownership', () => {
    expect(
      planScenarioChunk('/repo/src/features/workflow/WorkflowPage.tsx')
    ).toBeUndefined();
  });

  test('does not collapse dynamic runtime inventories into eager vendors', () => {
    expect(
      planScenarioChunk(
        '\0virtual:1flowbase-native-ant-design-icon-leaf/CloseOutlined'
      )
    ).toBeUndefined();
    expect(
      planScenarioChunk('/repo/node_modules/monaco-editor/esm/editor.js')
    ).toBeUndefined();
  });
});

// Exercise Vite's real pure-CSS-chunk removal rather than copying its transform.
test.each([true, false])(
  'scenario manifest final output integrity (late hook=%s)',
  async (lateHook) => {
    const { build } = await import('vite');
    const { scenarioChunkManifestPlugin } =
      await import('../scenario-chunk-planner');
    const sources: Record<string, string> = {
      '/scenario-final/main.js':
        "import './shared.css'; import('./secondary.js'); console.log('main');",
      '/scenario-final/secondary.js':
        "import './shared.css'; console.log('secondary');",
      '/scenario-final/shared.css': '.fixture { color: red }'
    };
    const plugin = scenarioChunkManifestPlugin();
    if (!lateHook) {
      // Counterexample: the former hook snapshots Vite's soon-to-be-removed CSS chunk.
      plugin.enforce = undefined;
      if (typeof plugin.generateBundle === 'object')
        plugin.generateBundle = plugin.generateBundle.handler;
    }
    const result = await build({
      configFile: false,
      logLevel: 'silent',
      plugins: [
        plugin,
        {
          name: 'scenario-final-fixture',
          resolveId(id, importer) {
            const file =
              id.startsWith('./') && importer
                ? `${importer.slice(0, importer.lastIndexOf('/') + 1)}${id.slice(2)}`
                : id;
            if (Object.hasOwn(sources, file)) return file;
          },
          load(id) {
            return sources[id];
          }
        }
      ],
      build: {
        write: false,
        minify: false,
        cssCodeSplit: true,
        rolldownOptions: {
          input: '/scenario-final/main.js',
          output: {
            codeSplitting: {
              groups: [{ name: 'shared-style', test: /shared\.css$/u }]
            }
          }
        }
      }
    });
    const bundle = Array.isArray(result) ? result[0] : result;
    if (!('output' in bundle))
      throw new Error('Expected completed Vite fixture build');
    const files = new Set(bundle.output.map((item) => item.fileName));
    const resource = bundle.output.find(
      (item) => item.fileName === 'scenario-chunk-manifest.json'
    );
    if (resource?.type !== 'asset')
      throw new Error('Expected scenario manifest');
    const manifest = JSON.parse(String(resource.source));
    expect(bundle.output.some((item) => item.fileName.endsWith('.css'))).toBe(
      true
    );
    const absent: string[] = [];
    for (const chunk of manifest.chunks) {
      for (const file of [
        chunk.file,
        ...chunk.imports,
        ...chunk.dynamicImports,
        ...chunk.css
      ]) {
        if (!files.has(file)) absent.push(file);
      }
    }
    expect(absent.length === 0).toBe(lateHook);
  }
);
