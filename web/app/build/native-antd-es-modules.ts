import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';

import type { Plugin } from 'vite';

export const NATIVE_ANTD_ES_MODULES_VIRTUAL_ID =
  'virtual:1flowbase-native-antd-es-modules';
export const NATIVE_ANTD_PRODUCT_LOADERS_VIRTUAL_ID =
  'virtual:1flowbase-native-antd-product-loaders';
export const NATIVE_ANTD_ES_LOADERS_VIRTUAL_ID =
  'virtual:1flowbase-native-antd-es-loaders';
export const PRODUCT_ANTD_RESOLVED_CHUNKS = new Map<string, string>();

const RESOLVED_VIRTUAL_ID = `\0${NATIVE_ANTD_ES_MODULES_VIRTUAL_ID}`;
const RESOLVED_PRODUCT_LOADERS_VIRTUAL_ID = `\0${NATIVE_ANTD_PRODUCT_LOADERS_VIRTUAL_ID}`;
const RESOLVED_ES_LOADERS_VIRTUAL_ID = `\0${NATIVE_ANTD_ES_LOADERS_VIRTUAL_ID}`;
const antdRequire = createRequire(import.meta.url);
const antdPackageRoot = path.dirname(antdRequire.resolve('antd/package.json'));
const productExportPaths = collectProductExportPaths();

interface AntDesignEsModuleSource {
  moduleSource: string;
  loaderSource: string;
}

export function nativeAntDesignEsModulesPlugin(
  command: 'build' | 'serve'
): Plugin {
  let moduleSources: readonly AntDesignEsModuleSource[] | undefined;

  return {
    name: '1flowbase-native-antd-es-modules',
    enforce: 'pre',
    transform: {
      filter: {
        id: { include: /\.[cm]?[jt]sx?$/u, exclude: /\/node_modules\//u },
        code: /antd/u
      },
      handler(source, id) {
        if (command === 'serve') return undefined;
        if (!/\.[cm]?[jt]sx?$/u.test(id) || id.includes('/node_modules/')) {
          return undefined;
        }
        const rewritten = rewriteProductAntDesignImports(source);
        return rewritten === source
          ? undefined
          : { code: rewritten, map: null };
      }
    },
    resolveId: {
      filter: {
        id: [
          /^virtual:1flowbase-native-antd-(?:es-modules|product-loaders|es-loaders)$/u,
          /^antd\/locale\/[A-Za-z0-9_]+(?:\.js)?$/u,
          /^antd\/es\/[A-Za-z0-9_-]+(?:\/.*)?$/u
        ]
      },
      async handler(id, importer) {
        if (id === NATIVE_ANTD_ES_MODULES_VIRTUAL_ID) {
          return RESOLVED_VIRTUAL_ID;
        }
        if (id === NATIVE_ANTD_PRODUCT_LOADERS_VIRTUAL_ID) {
          return RESOLVED_PRODUCT_LOADERS_VIRTUAL_ID;
        }
        if (id === NATIVE_ANTD_ES_LOADERS_VIRTUAL_ID) {
          return RESOLVED_ES_LOADERS_VIRTUAL_ID;
        }
        if (/^antd\/locale\/[A-Za-z0-9_]+(?:\.js)?$/u.test(id)) {
          moduleSources ??= collectAntDesignEsModuleSources();
          const locale = moduleSources.find(
            ({ moduleSource }) => moduleSource === id
          );
          if (locale) {
            return this.resolve(locale.loaderSource, importer, {
              skipSelf: true
            });
          }
        }
        if (
          isProductAntDesignSource(id) &&
          !importer?.includes('virtual:1flowbase-native-antd-')
        ) {
          const resolved = await this.resolve(id, importer, { skipSelf: true });
          if (!resolved) return undefined;
          const chunk = productAntDesignChunk(id);
          if (chunk) {
            PRODUCT_ANTD_RESOLVED_CHUNKS.set(resolved.id, chunk);
          }
          return resolved.id;
        }
        return undefined;
      }
    },
    load: {
      filter: {
        id: /virtual:1flowbase-native-antd-(?:es-modules|product-loaders|es-loaders)$/u
      },
      handler(id) {
        moduleSources ??= collectAntDesignEsModuleSources();
        if (id === RESOLVED_VIRTUAL_ID) {
          return renderNativeAntDesignEsModules(moduleSources);
        }
        if (id === RESOLVED_PRODUCT_LOADERS_VIRTUAL_ID) {
          return renderNativeAntDesignProductLoaders(moduleSources);
        }
        if (id === RESOLVED_ES_LOADERS_VIRTUAL_ID) {
          return renderNativeAntDesignDeepLoaders(moduleSources);
        }
        return undefined;
      }
    }
  };
}

function isProductAntDesignSource(id: string): boolean {
  return /^antd\/es\/[A-Za-z0-9_-]+(?:\/.*)?$/u.test(id);
}

function collectProductExportPaths(): ReadonlyMap<string, string> {
  const indexSource = readFileSync(
    path.join(antdPackageRoot, 'es/index.js'),
    'utf8'
  );
  return new Map(
    [
      ...indexSource.matchAll(
        /export \{ default as (\w+) \} from '\.\/([^']+)'/gu
      )
    ].map((match) => [match[1], match[2]])
  );
}

export function rewriteProductAntDesignImports(source: string): string {
  return source.replace(
    /import\s*\{([^}]*)\}\s*from\s*['"]antd['"];?/gu,
    (statement, clause: string) => {
      const types: string[] = [];
      const unresolved: string[] = [];
      const leaves: string[] = [];
      for (const rawSpecifier of clause.split(',')) {
        const specifier = rawSpecifier.trim();
        if (!specifier) continue;
        if (specifier.startsWith('type ')) {
          types.push(specifier.slice('type '.length).trim());
          continue;
        }
        const [exportName, localName = exportName] = specifier
          .split(/\s+as\s+/u)
          .map((value) => value.trim());
        const exportPath = productExportPaths.get(exportName);
        if (!exportPath) {
          unresolved.push(specifier);
          continue;
        }
        leaves.push(
          `import ${localName} from ${JSON.stringify(`antd/es/${exportPath}`)};`
        );
      }
      const rootImports = [
        ...(types.length > 0
          ? [`import type { ${types.join(', ')} } from 'antd';`]
          : []),
        ...(unresolved.length > 0
          ? [`import { ${unresolved.join(', ')} } from 'antd';`]
          : [])
      ];
      return [...rootImports, ...leaves].join('\n') || statement;
    }
  );
}

export function productAntDesignChunk(id: string): string | undefined {
  const component = /^antd\/es\/([^/]+)/u.exec(id)?.[1];
  if (!component) return 'antd-core';
  if (component === 'locale') {
    return /^antd\/es\/locale\/en_US(?:\.js)?$/u.test(id)
      ? 'antd-core'
      : undefined;
  }
  if (
    [
      'drawer',
      'dropdown',
      'modal',
      'popover',
      'popconfirm',
      'tooltip',
      'watermark'
    ].includes(component)
  )
    return 'antd-overlay';
  if (
    [
      'checkbox',
      'form',
      'input',
      'input-number',
      'radio',
      'segmented',
      'select',
      'switch'
    ].includes(component)
  )
    return 'antd-form';
  if (
    [
      'cascader',
      'descriptions',
      'list',
      'listy',
      'pagination',
      'table',
      'transfer',
      'tree',
      'tree-select',
      'upload'
    ].includes(component)
  )
    return 'antd-data';
  if (
    [
      'auto-complete',
      'calendar',
      'color-picker',
      'date-picker',
      'mentions',
      'rate',
      'slider',
      'time-picker'
    ].includes(component)
  )
    return 'antd-picker';
  if (
    [
      'alert',
      'avatar',
      'badge',
      'empty',
      'message',
      'notification',
      'progress',
      'result',
      'skeleton',
      'spin',
      'tag'
    ].includes(component)
  )
    return 'antd-feedback';
  if (['breadcrumb', 'menu', 'steps', 'tabs'].includes(component))
    return 'antd-navigation';
  if (
    ['card', 'collapse', 'image', 'splitter', 'statistic', 'timeline'].includes(component)
  )
    return 'antd-display';
  if (
    [
      'affix',
      'anchor',
      'back-top',
      'border-beam',
      'carousel',
      'float-button',
      'masonry',
      'qr-code',
      'tour'
    ].includes(component)
  )
    return undefined;
  return 'antd-core';
}

export function collectAntDesignEsModuleSources(
  packageRoot = antdPackageRoot
): AntDesignEsModuleSource[] {
  const esRoot = path.join(packageRoot, 'es');
  const bySource = new Map<string, string>();

  for (const relativeFile of listJavaScriptFiles(esRoot)) {
    const loaderSource = `antd/es/${relativeFile}`;
    const withoutExtension = loaderSource.slice(0, -'.js'.length);
    bySource.set(loaderSource, loaderSource);
    bySource.set(withoutExtension, loaderSource);
    // Public locale entries share the installed ESM implementation.
    if (
      /^locale\/[A-Za-z0-9_]+\.js$/u.test(relativeFile) &&
      existsSync(path.join(packageRoot, relativeFile))
    ) {
      bySource.set(`antd/${relativeFile}`, loaderSource);
      bySource.set(
        `antd/${relativeFile.slice(0, -'.js'.length)}`,
        loaderSource
      );
    }
    if (relativeFile.endsWith('/index.js')) {
      bySource.set(
        `antd/es/${relativeFile.slice(0, -'/index.js'.length)}`,
        loaderSource
      );
    }
  }

  return [...bySource]
    .map(([moduleSource, loaderSource]) => ({ moduleSource, loaderSource }))
    .sort((left, right) => left.moduleSource.localeCompare(right.moduleSource));
}

function listJavaScriptFiles(directory: string, prefix = ''): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const relativePath = `${prefix}${entry.name}`;
    if (entry.isDirectory()) {
      return listJavaScriptFiles(
        path.join(directory, entry.name),
        `${relativePath}/`
      );
    }
    return entry.isFile() && entry.name.endsWith('.js') ? [relativePath] : [];
  });
}

function partitionProductAndDeepSources(
  moduleSources: readonly AntDesignEsModuleSource[]
) {
  const productLoaderSources = new Set<string>();
  for (const exportPath of productExportPaths.values()) {
    productLoaderSources.add(`antd/es/${exportPath}.js`);
    productLoaderSources.add(`antd/es/${exportPath}/index.js`);
  }
  productLoaderSources.add('antd/es/modal/useModal.js');
  productLoaderSources.add('antd/es/modal/useModal/index.js');

  const productEntries: AntDesignEsModuleSource[] = [];
  const deepEntries: AntDesignEsModuleSource[] = [];
  for (const entry of moduleSources) {
    if (
      productLoaderSources.has(entry.loaderSource) ||
      /^antd\/es\/locale\/[A-Za-z0-9_]+\.js$/u.test(entry.loaderSource)
    ) {
      productEntries.push(entry);
    } else {
      deepEntries.push(entry);
    }
  }
  return { productEntries, deepEntries };
}

function renderDeduplicatedLoaderMap(
  entries: readonly AntDesignEsModuleSource[],
  exportFnName: string
): string {
  const canonicalSources = [
    ...new Set(entries.map(({ loaderSource }) => loaderSource))
  ].sort((left, right) => left.localeCompare(right));
  const aliasPairs = entries
    .filter(({ loaderSource, moduleSource }) => moduleSource !== loaderSource)
    .map(({ loaderSource, moduleSource }) => [moduleSource, loaderSource] as const);
  return `
const canonicalLoaders = {${canonicalSources
    .map(
      (loaderSource) =>
        `\n  ${JSON.stringify(loaderSource)}: () => import(${JSON.stringify(loaderSource)}),`
    )
    .join('')}\n};
const aliasToCanonical = ${JSON.stringify(Object.fromEntries(aliasPairs))};
const moduleFlights = new Map();

export async function ${exportFnName}(moduleSource) {
  const current = moduleFlights.get(moduleSource);
  if (current) return current;
  const canonical = aliasToCanonical[moduleSource] ?? moduleSource;
  const load = canonicalLoaders[canonical];
  if (!load) throw new Error('Ant Design ES module is not installed: ' + moduleSource + '.');
  const flight = load().catch((error) => {
    if (moduleFlights.get(moduleSource) === flight) moduleFlights.delete(moduleSource);
    throw error;
  });
  moduleFlights.set(moduleSource, flight);
  return flight;
}
`;
}

function renderNativeAntDesignProductLoaders(
  moduleSources: readonly AntDesignEsModuleSource[]
): string {
  const { productEntries } = partitionProductAndDeepSources(moduleSources);
  return `${renderDeduplicatedLoaderMap(productEntries, 'loadProductAntDesignEsModule')}
let rootModulePromise;
export function loadAntDesignRootModuleFromDomain() {
  rootModulePromise ??= import('antd').catch((error) => {
    rootModulePromise = undefined;
    throw error;
  });
  return rootModulePromise;
}
`;
}

function renderNativeAntDesignDeepLoaders(
  moduleSources: readonly AntDesignEsModuleSource[]
): string {
  const { deepEntries } = partitionProductAndDeepSources(moduleSources);
  return renderDeduplicatedLoaderMap(deepEntries, 'loadDeepAntDesignEsModule');
}

function renderNativeAntDesignEsModules(
  moduleSources: readonly AntDesignEsModuleSource[]
): string {
  const rootExports = Object.keys(
    antdRequire('antd') as Record<string, unknown>
  ).sort((left, right) => left.localeCompare(right));
  const { productEntries, deepEntries } =
    partitionProductAndDeepSources(moduleSources);
  const productExportMap = Object.fromEntries(
    [...productExportPaths.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([exportName, exportPath]) => [exportName, `antd/es/${exportPath}`])
  );
  return `
const productSources = new Set(${JSON.stringify(productEntries.map(({ moduleSource }) => moduleSource))});
const deepSources = new Set(${JSON.stringify(deepEntries.map(({ moduleSource }) => moduleSource))});

export const ANTD_PRODUCT_EXPORT_PATHS = Object.freeze(${JSON.stringify(productExportMap)});

export const ANTD_ES_MODULE_DEFINITIONS = Object.freeze(
  ${JSON.stringify(moduleSources.map(({ moduleSource }) => moduleSource))}.map((module_source) => ({ module_source, exports: ['*'] }))
);

export const ANTD_ROOT_EXPORTS = Object.freeze(${JSON.stringify(rootExports)});

let productDomainPromise;
function loadProductDomain() {
  productDomainPromise ??= import(${JSON.stringify(NATIVE_ANTD_PRODUCT_LOADERS_VIRTUAL_ID)}).catch((error) => {
    productDomainPromise = undefined;
    throw error;
  });
  return productDomainPromise;
}

let deepDomainPromise;
function loadDeepDomain() {
  deepDomainPromise ??= import(${JSON.stringify(NATIVE_ANTD_ES_LOADERS_VIRTUAL_ID)}).catch((error) => {
    deepDomainPromise = undefined;
    throw error;
  });
  return deepDomainPromise;
}

export function prewarmAntDesignProductDomain() {
  return loadProductDomain();
}

export async function loadAntDesignRootModule() {
  const domain = await loadProductDomain();
  return domain.loadAntDesignRootModuleFromDomain();
}

export async function loadAntDesignEsModule(moduleSource) {
  if (productSources.has(moduleSource)) {
    const domain = await loadProductDomain();
    return domain.loadProductAntDesignEsModule(moduleSource);
  }
  if (deepSources.has(moduleSource)) {
    const domain = await loadDeepDomain();
    return domain.loadDeepAntDesignEsModule(moduleSource);
  }
  throw new Error('Ant Design ES module is not installed: ' + moduleSource + '.');
}
`;
}
