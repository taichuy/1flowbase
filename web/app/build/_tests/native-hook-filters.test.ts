import { build, createServer, type Plugin } from 'vite';
import { describe, expect, test, vi } from 'vitest';

import {
  HMR_PROBE_ID,
  oneFlowbaseDevRuntimePlugin
} from '../../vite/dev-runtime';
import {
  NATIVE_ANT_DESIGN_ICON_LEAF_VIRTUAL_PREFIX,
  nativeAntDesignIconsModulesPlugin
} from '../native-ant-design-icons-modules';
import { nativeAntDesignEsModulesPlugin } from '../native-antd-es-modules';
import {
  NATIVE_DAYJS_LEAF_VIRTUAL_PREFIX,
  nativeDayjsModulesPlugin
} from '../native-dayjs-modules';
import {
  NATIVE_DND_KIT_LEAF_VIRTUAL_PREFIX,
  nativeDndKitModulesPlugin
} from '../native-dnd-kit-modules';
import {
  NATIVE_MODULE_DECLARATIONS_VIRTUAL_ID,
  nativeModuleDeclarationsPlugin
} from '../native-module-declarations';

function traceHooks(plugin: Plugin) {
  const resolveHook = plugin.resolveId;
  const loadHook = plugin.load;
  if (!resolveHook || typeof resolveHook !== 'object') {
    throw new Error('Expected a filtered resolveId hook');
  }
  if (!loadHook || typeof loadHook !== 'object') {
    throw new Error('Expected a filtered load hook');
  }
  return {
    resolveId: vi.spyOn(resolveHook, 'handler'),
    load: vi.spyOn(loadHook, 'handler')
  };
}

async function buildFixture(
  plugins: Plugin[],
  source: string,
  modules: Record<string, string> = {}
) {
  const entry = '/native-filter-fixture/entry.js';
  const sources: Record<string, string> = { ...modules, [entry]: source };
  const result = await build({
    configFile: false,
    logLevel: 'silent',
    plugins: [
      ...plugins,
      {
        name: 'native-filter-fixture',
        resolveId(id) {
          if (Object.hasOwn(sources, id)) return id;
        },
        load(id) {
          return sources[id];
        }
      }
    ],
    build: {
      write: false,
      minify: false,
      lib: { entry, formats: ['es'], fileName: 'native-filter-fixture' }
    }
  });
  const output = Array.isArray(result) ? result[0] : result;
  if (!('output' in output)) throw new Error('Expected a completed build');
  return output.output
    .filter((item) => item.type === 'chunk')
    .map((item) => item.code)
    .join('\n');
}

function leafFixtures() {
  return [
    {
      name: 'dayjs',
      prefix: NATIVE_DAYJS_LEAF_VIRTUAL_PREFIX,
      moduleSource: 'dayjs/plugin/utc',
      plugin: nativeDayjsModulesPlugin({
        inventory: [
          {
            moduleSource: 'dayjs/plugin/utc',
            loaderSource: 'dayjs/plugin/utc.js',
            devLoaderSource: 'dayjs/esm/plugin/utc/index.js',
            hasDeclaration: true,
            packageName: 'dayjs',
            packageVersion: 'fixture'
          }
        ]
      }),
      loaderSource: 'dayjs/plugin/utc.js'
    },
    {
      name: 'dnd-kit',
      prefix: NATIVE_DND_KIT_LEAF_VIRTUAL_PREFIX,
      moduleSource: '@dnd-kit/core',
      plugin: nativeDndKitModulesPlugin({
        inventory: [
          {
            moduleSource: '@dnd-kit/core',
            loaderSource: '@dnd-kit/core',
            devLoaderSource: '@dnd-kit/core/dist/core.esm.js',
            packageName: '@dnd-kit/core',
            packageVersion: 'fixture'
          }
        ]
      }),
      loaderSource: '@dnd-kit/core'
    },
    {
      name: 'icons',
      prefix: NATIVE_ANT_DESIGN_ICON_LEAF_VIRTUAL_PREFIX,
      moduleSource: 'ClockCircleOutlined',
      plugin: nativeAntDesignIconsModulesPlugin({
        inventory: {
          modules: [
            {
              moduleSource: '@ant-design/icons/ClockCircleOutlined',
              loaderSource: '@ant-design/icons/ClockCircleOutlined'
            }
          ],
          packageName: '@ant-design/icons',
          packageVersion: 'fixture',
          rootExports: ['ClockCircleOutlined']
        }
      }),
      loaderSource: '@ant-design/icons/ClockCircleOutlined'
    }
  ];
}

describe('native Vite hook dispatch filters', () => {
  test('does not invoke native module handlers for unrelated or lookalike IDs', async () => {
    const plugins = [
      ...leafFixtures().map(({ plugin }) => plugin),
      nativeAntDesignEsModulesPlugin('build'),
      nativeModuleDeclarationsPlugin({
        moduleSources: [],
        projectRoot: process.cwd()
      }),
      oneFlowbaseDevRuntimePlugin({
        root: process.cwd(),
        mode: 'test',
        command: 'build'
      })
    ];
    const traces = plugins.map(traceHooks);
    const ids = [
      '/native-filter-fixture/plain.js',
      'virtual:1flowbase-dev-hmr-probe-extra',
      'virtual:1flowbase-native-module-declarations-extra',
      'virtual:1flowbase-native-antd-es-modules-extra',
      'virtual:1flowbase-native-ant-design-icons-modules-extra',
      'virtual:1flowbase-native-dayjs-modules-extra',
      'virtual:1flowbase-native-dnd-kit-modules-extra',
      '\0virtual:1flowbase-native-dayjs-modules-extra',
      '\0virtual:1flowbase-native-dnd-kit-modules-extra',
      '\0virtual:1flowbase-native-ant-design-icons-loaders-extra',
      '\0virtual:1flowbase-native-antd-es-modules-extra',
      '\0virtual:1flowbase-native-module-declarations-extra',
      '\0virtual:1flowbase-dev-hmr-probe-extra'
    ];
    await buildFixture(
      plugins,
      ids.map((id) => `import ${JSON.stringify(id)};`).join('\n') +
        '\nexport const value = 42;',
      Object.fromEntries(ids.map((id) => [id, 'export const untouched = 1;']))
    );
    for (const trace of traces) {
      expect(trace.resolveId).not.toHaveBeenCalled();
      expect(trace.load).not.toHaveBeenCalled();
    }
  });

  test('dispatches exact dev-probe and declaration roots to their canonical NUL loads', async () => {
    const plugins = [
      oneFlowbaseDevRuntimePlugin({
        root: process.cwd(),
        mode: 'test',
        command: 'build'
      }),
      nativeModuleDeclarationsPlugin({
        moduleSources: [],
        projectRoot: process.cwd()
      })
    ];
    const traces = plugins.map(traceHooks);
    const ids = [HMR_PROBE_ID, NATIVE_MODULE_DECLARATIONS_VIRTUAL_ID];
    const code = await buildFixture(
      plugins,
      `
      import ${JSON.stringify(HMR_PROBE_ID)};
      export { default as declarations } from ${JSON.stringify(NATIVE_MODULE_DECLARATIONS_VIRTUAL_ID)};
    `
    );
    expect(code).toContain('declarations');
    for (const [index, id] of ids.entries()) {
      expect(
        traces[index].resolveId.mock.calls.map(([source]) => source)
      ).toEqual([id]);
      expect(traces[index].load.mock.calls.map(([source]) => source)).toEqual([
        `\0${id}`
      ]);
    }
  });

  test.each(leafFixtures())(
    '$name preserves valid leaf loads and rejects unknown leaves',
    async ({ name, plugin, prefix, moduleSource, loaderSource }) => {
      const trace = traceHooks(plugin);
      const leafId = `${prefix}${moduleSource}`;
      const code = await buildFixture(
        [plugin],
        `export { ${name === 'icons' ? 'default' : 'moduleNamespace'} } from ${JSON.stringify(leafId)};`,
        { [loaderSource]: 'export default "native-leaf-sentinel";' }
      );
      expect(code).toContain('native-leaf-sentinel');
      expect(trace.resolveId.mock.calls.map(([id]) => id)).toContain(leafId);
      expect(trace.load.mock.calls.map(([id]) => id)).toContain(`\0${leafId}`);
      await expect(
        buildFixture(
          [plugin],
          `export * from ${JSON.stringify(`${prefix}not-installed`)};`
        )
      ).rejects.toThrow(/not installed or (?:public|resolvable)/u);
      expect(trace.resolveId.mock.calls.map(([id]) => id)).toContain(
        `${prefix}not-installed`
      );
    }
  );

  test('keeps dayjs serve aliases and build package imports distinct', async () => {
    const aliases = [
      'dayjs',
      'dayjs/plugin/utc',
      'dayjs/plugin/utc.js',
      'dayjs/locale/en',
      'dayjs/locale/en.js'
    ];
    const canonical = [
      'dayjs/esm/index.js',
      'dayjs/esm/plugin/utc/index.js',
      'dayjs/esm/plugin/utc/index.js',
      'dayjs/esm/locale/en.js',
      'dayjs/esm/locale/en.js'
    ];
    const inventory = aliases.map((moduleSource, index) => ({
      moduleSource,
      devLoaderSource: canonical[index],
      loaderSource: moduleSource,
      hasDeclaration: true,
      packageName: 'dayjs',
      packageVersion: 'fixture'
    }));
    const plugin = nativeDayjsModulesPlugin({ inventory });
    const trace = traceHooks(plugin);
    const observed: Array<{
      id: string;
      importer: string | undefined;
      custom: unknown;
    }> = [];
    const server = await createServer({
      configFile: false,
      logLevel: 'silent',
      appType: 'custom',
      server: { middlewareMode: true, hmr: false },
      optimizeDeps: { noDiscovery: true, include: [] },
      plugins: [
        plugin,
        {
          name: 'dayjs-alias-fixture',
          enforce: 'pre',
          resolveId(id, importer, options) {
            if (!canonical.includes(id)) return;
            observed.push({ id, importer, custom: options.custom });
            return `\0fixture:${id}`;
          }
        }
      ]
    });
    try {
      const importer = '/native-filter-fixture/consumer.ts';
      const custom = { 'native-filter-fixture': { retained: true } };
      for (const [index, alias] of aliases.entries()) {
        const resolved =
          await server.environments.client.pluginContainer.resolveId(
            alias,
            importer,
            { custom }
          );
        expect(resolved?.id).toBe(`\0fixture:${canonical[index]}`);
      }
      expect(observed).toEqual(
        canonical.map((id) => ({ id, importer, custom }))
      );
      expect(trace.resolveId.mock.calls.map(([id]) => id)).toEqual(aliases);
    } finally {
      await server.close();
    }
    const buildPlugin = nativeDayjsModulesPlugin({ inventory });
    const code = await buildFixture(
      [buildPlugin],
      aliases
        .map(
          (id, index) =>
            `export { default as value${index} } from ${JSON.stringify(id)};`
        )
        .join('\n'),
      Object.fromEntries(
        aliases.map((id) => [id, 'export default "build-package-import";'])
      )
    );
    expect(code).toContain('build-package-import');
  });

  test('dispatches AntD transforms only for local script IDs containing antd', async () => {
    const plugin = nativeAntDesignEsModulesPlugin('build');
    const hook = plugin.transform;
    if (!hook || typeof hook !== 'object')
      throw new Error('Expected a filtered transform hook');
    const transform = vi.spyOn(hook, 'handler');
    const included = ['ts', 'tsx', 'mjs', 'cjs'].map(
      (extension) => `/native-filter-fixture/local.${extension}`
    );
    const excluded = [
      '/native-filter-fixture/no-match.js',
      '/native-filter-fixture/node_modules/dependency.js',
      '/native-filter-fixture/query.js?raw'
    ];
    const modules = Object.fromEntries([
      ...included.map((id) => [id, 'export const value = "antd";']),
      [excluded[0], 'export const value = "other";'],
      ...excluded.slice(1).map((id) => [id, 'export const value = "antd";'])
    ]);
    await buildFixture(
      [plugin],
      [...included, ...excluded]
        .map((id) => `import ${JSON.stringify(id)};`)
        .join('\n') + '\nexport const value = 1;',
      modules
    );
    expect(transform.mock.calls.map(([, id]) => id).sort()).toEqual(
      included.sort()
    );
  });
});
