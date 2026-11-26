import { describe, expect, test } from 'vitest';

import {
  planScenarioChunk,
  scenarioCodeSplitting
} from '../scenario-chunk-planner';
import { productAntDesignChunk } from '../native-antd-es-modules';

describe('scenario chunk planner', () => {
  test('coalesces authenticated vendor runtimes into app-runtime while keeping dayjs locales and antd-picker lazy', () => {
    const appRuntimeGroup = scenarioCodeSplitting.groups.find(
      (group) => 'name' in group && group.name === 'app-runtime'
    );
    expect(appRuntimeGroup?.priority).toBe(7);
    const pattern =
      appRuntimeGroup &&
      'test' in appRuntimeGroup &&
      appRuntimeGroup.test instanceof RegExp
        ? appRuntimeGroup.test
        : null;
    expect(pattern).not.toBeNull();
    expect(pattern?.test('/repo/node_modules/dayjs/dayjs.min.js')).toBe(true);
    expect(
      pattern?.test('/repo/node_modules/dayjs/plugin/customParseFormat.js')
    ).toBe(true);
    expect(pattern?.test('/repo/node_modules/dayjs/locale/zh-cn.js')).toBe(
      false
    );
    expect(pattern?.test('/repo/node_modules/dayjs/esm/locale/zh-cn.js')).toBe(
      false
    );
    expect(
      pattern?.test('/repo/node_modules/@tanstack/react-query/build/modern/index.js')
    ).toBe(true);
    expect(
      pattern?.test('/repo/node_modules/@tanstack/react-router/dist/esm/index.js')
    ).toBe(true);
    expect(pattern?.test('/repo/node_modules/i18next/dist/esm/i18next.js')).toBe(
      true
    );
    expect(
      pattern?.test('/repo/node_modules/antd/es/date-picker/locale/zh_CN.js')
    ).toBe(true);
    expect(
      pattern?.test(
        '/repo/node_modules/@rc-component/picker/es/locale/zh_CN.js'
      )
    ).toBe(true);
    expect(
      pattern?.test('/repo/node_modules/antd/es/date-picker/index.js')
    ).toBe(false);
  });

  test('coalesces scenario quotient groups and virtual loader tables without pulling editor or heavy widget chunks', () => {
    const findPattern = (groupName: string) => {
      const found = scenarioCodeSplitting.groups.find(
        (group) => 'name' in group && group.name === groupName
      );
      return found && 'test' in found && found.test instanceof RegExp
        ? found.test
        : null;
    };

    const shellZh = findPattern('i18n-shell-zh');
    expect(shellZh?.test('/repo/web/app/src/app-shell/i18n/zh_Hans.json')).toBe(
      true
    );
    expect(
      shellZh?.test('/repo/web/app/src/features/settings/i18n/zh_Hans.json')
    ).toBe(false);
    expect(
      shellZh?.test('/repo/web/app/src/features/frontstage/i18n/zh_Hans.json')
    ).toBe(false);
    expect(
      shellZh?.test(
        '/repo/web/app/src/features/notifications/i18n/zh_Hans.json'
      )
    ).toBe(false);
    expect(
      shellZh?.test(
        '/repo/web/app/src/features/settings/components/billing/i18n/zh_Hans.json'
      )
    ).toBe(false);

    const authVisual = findPattern('auth-visual');
    expect(
      authVisual?.test('/repo/node_modules/antd/es/config-provider/index.js')
    ).toBe(true);
    expect(
      authVisual?.test('/repo/node_modules/antd/es/skeleton/index.js')
    ).toBe(false);

    const shellCore = findPattern('shell-core');
    expect(
      shellCore?.test('/repo/web/app/src/app-shell/AppShellFrame.tsx')
    ).toBe(true);
    expect(
      shellCore?.test(
        '/repo/web/app/src/app/bootstrap/AuthenticatedStartup.tsx'
      )
    ).toBe(true);
    expect(
      shellCore?.test(
        '/repo/web/app/src/app-shell/TopbarNavigationDesigner.tsx'
      )
    ).toBe(false);
    expect(
      shellCore?.test('/repo/web/app/src/routes/LazyRouteResult.tsx')
    ).toBe(false);
    expect(
      shellCore?.test(
        '/repo/web/app/src/shared/ui/section-page-layout/SectionPageLayout.tsx'
      )
    ).toBe(false);

    const nativeHost = findPattern('native-host');
    expect(
      nativeHost?.test(
        '/repo/web/packages/page-runtime/src/native-react-compiler/artifact-evaluator.ts'
      )
    ).toBe(true);
    expect(
      nativeHost?.test(
        '\0virtual:1flowbase-native-antd-product-loaders'
      )
    ).toBe(true);
    expect(
      nativeHost?.test(
        '\0virtual:1flowbase-native-ant-design-icons-loaders'
      )
    ).toBe(true);
    expect(
      nativeHost?.test(
        '/repo/web/app/src/features/frontstage/lib/native-modules/overlay/native-date-picker-runtime.tsx'
      )
    ).toBe(true);
    expect(
      nativeHost?.test(
        '/repo/web/app/src/features/frontstage/lib/native-modules/overlay/native-select-runtime.tsx'
      )
    ).toBe(false);
    expect(
      nativeHost?.test(
        '/repo/web/app/src/features/frontstage/lib/native-modules/charts/runtime.tsx'
      )
    ).toBe(false);
    expect(
      nativeHost?.test(
        '/repo/web/packages/page-runtime/src/native-trusted-block/source-scanner.ts'
      )
    ).toBe(false);

    const frontstageShell = findPattern('frontstage-shell');
    const frontstageCanvas = findPattern('frontstage-canvas');
    expect(
      frontstageShell?.test(
        '/repo/web/app/src/features/frontstage/pages/FrontstageWorkspacePage.tsx'
      )
    ).toBe(true);
    expect(
      frontstageShell?.test(
        '/repo/web/app/src/features/frontstage/components/FrontStagePageTreeSidebar.tsx'
      )
    ).toBe(true);
    expect(
      frontstageShell?.test(
        '/repo/web/app/src/features/frontstage/pages/FrontStagePage.tsx'
      )
    ).toBe(false);
    expect(
      frontstageShell?.test(
        '/repo/web/app/src/features/frontstage/lib/page-tree-icons/PageTreeIconPicker.tsx'
      )
    ).toBe(false);
    expect(
      frontstageCanvas?.test(
        '/repo/web/app/src/features/frontstage/pages/FrontStagePage.tsx'
      )
    ).toBe(true);
    expect(
      frontstageCanvas?.test(
        '/repo/web/app/src/features/frontstage/components/PageCanvas.tsx'
      )
    ).toBe(true);
  });
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
    ['antd/es/transfer', 'antd-data'],
    ['antd/es/result', 'antd-feedback']
  ])('classifies product source %s as %s', (source, chunk) => {
    expect(productAntDesignChunk(source)).toBe(chunk);
  });

  test('orders antd-data strictly above antd-picker so shared Pagination never forces Table to import antd-picker', () => {
    const dataGroup = scenarioCodeSplitting.groups.find(
      (group) => 'debugName' in group && group.debugName === 'antd-data'
    );
    const pickerGroup = scenarioCodeSplitting.groups.find(
      (group) => 'debugName' in group && group.debugName === 'antd-picker'
    );
    expect(dataGroup?.priority).toBe(1);
    expect(pickerGroup?.priority).toBe(0);
    expect(dataGroup!.priority!).toBeGreaterThan(pickerGroup!.priority!);
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
