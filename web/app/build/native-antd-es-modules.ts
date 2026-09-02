import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';

import type { Plugin } from 'vite';

export const NATIVE_ANTD_ES_MODULES_VIRTUAL_ID =
  'virtual:1flowbase-native-antd-es-modules';
export const PRODUCT_ANTD_RESOLVED_CHUNKS = new Map<string, string>();

const RESOLVED_VIRTUAL_ID = `\0${NATIVE_ANTD_ES_MODULES_VIRTUAL_ID}`;
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
    transform(source, id) {
      if (command === 'serve') return undefined;
      if (!/\.[cm]?[jt]sx?$/u.test(id) || id.includes('/node_modules/')) {
        return undefined;
      }
      const rewritten = rewriteProductAntDesignImports(source);
      return rewritten === source ? undefined : { code: rewritten, map: null };
    },
    async resolveId(id, importer) {
      if (id === NATIVE_ANTD_ES_MODULES_VIRTUAL_ID) {
        return RESOLVED_VIRTUAL_ID;
      }
      if (
        isProductAntDesignSource(id) &&
        !importer?.includes(NATIVE_ANTD_ES_MODULES_VIRTUAL_ID)
      ) {
        const resolved = await this.resolve(id, importer, { skipSelf: true });
        if (!resolved) return undefined;
        PRODUCT_ANTD_RESOLVED_CHUNKS.set(
          resolved.id,
          productAntDesignChunk(id)
        );
        return resolved.id;
      }
      return undefined;
    },
    load(id) {
      if (id !== RESOLVED_VIRTUAL_ID) return undefined;
      moduleSources ??= collectAntDesignEsModuleSources();
      return renderNativeAntDesignEsModules(moduleSources);
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

export function productAntDesignChunk(id: string): string {
  const component = /^antd\/es\/([^/]+)/u.exec(id)?.[1];
  if (!component) return 'antd-core';
  if (
    [
      'drawer',
      'dropdown',
      'modal',
      'popover',
      'popconfirm',
      'tooltip'
    ].includes(component)
  )
    return 'antd-overlay';
  if (
    [
      'calendar',
      'cascader',
      'date-picker',
      'descriptions',
      'form',
      'pagination',
      'select',
      'table',
      'time-picker',
      'tree',
      'tree-select',
      'upload'
    ].includes(component)
  )
    return 'antd-data';
  if (
    [
      'alert',
      'message',
      'notification',
      'progress',
      'result',
      'skeleton',
      'spin'
    ].includes(component)
  )
    return 'antd-feedback';
  if (['anchor', 'breadcrumb', 'menu', 'steps', 'tabs'].includes(component))
    return 'antd-navigation';
  if (
    [
      'checkbox',
      'color-picker',
      'input',
      'input-number',
      'mentions',
      'radio',
      'rate',
      'segmented',
      'slider',
      'switch',
      'transfer'
    ].includes(component)
  )
    return 'antd-input';
  if (
    [
      'avatar',
      'badge',
      'card',
      'carousel',
      'collapse',
      'empty',
      'image',
      'list',
      'statistic',
      'tag',
      'timeline',
      'tour'
    ].includes(component)
  )
    return 'antd-display';
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

function renderNativeAntDesignEsModules(
  moduleSources: readonly AntDesignEsModuleSource[]
): string {
  const rootExports = Object.keys(
    antdRequire('antd') as Record<string, unknown>
  ).sort((left, right) => left.localeCompare(right));
  const loaders = Object.fromEntries(
    moduleSources.map(({ moduleSource, loaderSource }) => [
      moduleSource,
      `() => import(${JSON.stringify(loaderSource)})`
    ])
  );
  return `
const loaders = {${Object.entries(loaders)
    .map(
      ([moduleSource, loader]) =>
        `\n  ${JSON.stringify(moduleSource)}: ${loader},`
    )
    .join('')}\n};

export const ANTD_ES_MODULE_DEFINITIONS = Object.freeze(
  Object.keys(loaders).map((module_source) => ({ module_source, exports: ['*'] }))
);

export const ANTD_ROOT_EXPORTS = Object.freeze(${JSON.stringify(rootExports)});

let rootModulePromise;

export function loadAntDesignRootModule() {
  rootModulePromise ??= import('antd').catch((error) => {
    rootModulePromise = undefined;
    throw error;
  });
  return rootModulePromise;
}

export async function loadAntDesignEsModule(moduleSource) {
  const load = loaders[moduleSource];
  if (!load) throw new Error('Ant Design ES module is not installed: ' + moduleSource + '.');
  return load();
}
`;
}
