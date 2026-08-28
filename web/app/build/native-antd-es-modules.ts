import { readdirSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';

import type { Plugin } from 'vite';

export const NATIVE_ANTD_ES_MODULES_VIRTUAL_ID =
  'virtual:1flowbase-native-antd-es-modules';

const RESOLVED_VIRTUAL_ID = `\0${NATIVE_ANTD_ES_MODULES_VIRTUAL_ID}`;
const antdRequire = createRequire(import.meta.url);

interface AntDesignEsModuleSource {
  moduleSource: string;
  loaderSource: string;
}

export function nativeAntDesignEsModulesPlugin(): Plugin {
  let moduleSources: readonly AntDesignEsModuleSource[] | undefined;

  return {
    name: '1flowbase-native-antd-es-modules',
    enforce: 'pre',
    resolveId(id) {
      return id === NATIVE_ANTD_ES_MODULES_VIRTUAL_ID
        ? RESOLVED_VIRTUAL_ID
        : undefined;
    },
    load(id) {
      if (id !== RESOLVED_VIRTUAL_ID) return undefined;
      moduleSources ??= collectAntDesignEsModuleSources();
      return renderNativeAntDesignEsModules(moduleSources);
    }
  };
}

export function collectAntDesignEsModuleSources(
  packageRoot = path.dirname(antdRequire.resolve('antd/package.json'))
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
      return listJavaScriptFiles(path.join(directory, entry.name), `${relativePath}/`);
    }
    return entry.isFile() && entry.name.endsWith('.js') ? [relativePath] : [];
  });
}

function renderNativeAntDesignEsModules(
  moduleSources: readonly AntDesignEsModuleSource[]
): string {
  const loaders = Object.fromEntries(
    moduleSources.map(({ moduleSource, loaderSource }) => [
      moduleSource,
      `() => import(${JSON.stringify(loaderSource)})`
    ])
  );
  return `
const loaders = {${Object.entries(loaders)
  .map(([moduleSource, loader]) => `\n  ${JSON.stringify(moduleSource)}: ${loader},`)
  .join('')}\n};

export const ANTD_ES_MODULE_DEFINITIONS = Object.freeze(
  Object.keys(loaders).map((module_source) => ({ module_source, exports: ['*'] }))
);

export async function loadAntDesignEsModule(moduleSource) {
  const load = loaders[moduleSource];
  if (!load) throw new Error('Ant Design ES module is not installed: ' + moduleSource + '.');
  return load();
}
`;
}
