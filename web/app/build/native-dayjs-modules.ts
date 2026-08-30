import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

import type { Plugin } from 'vite';

import {
  createDemandResolvedModuleDomain,
  generateDemandResolvedLoaderRuntime
} from './native-demand-resolved-modules';

export const NATIVE_DAYJS_MODULES_VIRTUAL_ID =
  'virtual:1flowbase-native-dayjs-modules';
export const NATIVE_DAYJS_LEAF_VIRTUAL_PREFIX =
  'virtual:1flowbase-native-dayjs-leaf/';

const RESOLVED_VIRTUAL_ID = `\0${NATIVE_DAYJS_MODULES_VIRTUAL_ID}`;
const DAYJS_PACKAGE_NAME = 'dayjs';
const JAVASCRIPT_EXTENSIONS = ['.js', '.mjs', '.cjs'] as const;

export interface DayjsModuleSource {
  devLoaderSource: string;
  hasDeclaration: boolean;
  loaderSource: string;
  moduleSource: string;
  packageName: string;
  packageVersion: string;
}

export function nativeDayjsModulesPlugin({
  inventory
}: {
  inventory: readonly DayjsModuleSource[];
}): Plugin {
  let command: 'build' | 'serve' = 'build';
  const demandDomain = createDemandResolvedModuleDomain({
    errorLabel: 'dayjs module',
    modules: inventory,
    virtualPrefix: NATIVE_DAYJS_LEAF_VIRTUAL_PREFIX
  });
  return {
    name: '1flowbase-native-dayjs-modules',
    enforce: 'pre',
    configResolved(config) {
      command = config.command;
    },
    resolveId(id) {
      return id === NATIVE_DAYJS_MODULES_VIRTUAL_ID
        ? RESOLVED_VIRTUAL_ID
        : demandDomain.resolveId(id);
    },
    load(id) {
      if (id === RESOLVED_VIRTUAL_ID) {
        return command === 'serve'
          ? generateNativeDayjsDevModules(inventory)
          : renderNativeDayjsModules(inventory);
      }
      return demandDomain.load(id, command);
    }
  };
}

export function collectDayjsModuleSources({
  projectRoot
}: {
  projectRoot: string;
}): DayjsModuleSource[] {
  const packageRoot = path.join(
    projectRoot,
    'node_modules',
    DAYJS_PACKAGE_NAME
  );
  const manifest = readPackageManifest(packageRoot);
  const inventory = new Map<string, DayjsModuleSource>();
  const rootEntry: DayjsModuleSource = {
    devLoaderSource: 'dayjs/esm/index.js',
    hasDeclaration: true,
    loaderSource: DAYJS_PACKAGE_NAME,
    moduleSource: DAYJS_PACKAGE_NAME,
    packageName: manifest.name,
    packageVersion: manifest.version
  };
  inventory.set(rootEntry.moduleSource, rootEntry);

  for (const relativeFile of listJavaScriptFiles(packageRoot)) {
    const loaderSource = `${DAYJS_PACKAGE_NAME}/${relativeFile}`;
    const aliases = new Set([
      loaderSource,
      withoutJavaScriptExtension(loaderSource)
    ]);
    if (relativeFile.endsWith('/index.js')) {
      aliases.add(loaderSource.slice(0, -'/index.js'.length));
    }
    const hasDeclaration = hasAdjacentDeclaration(packageRoot, relativeFile);
    for (const moduleSource of aliases) {
      inventory.set(moduleSource, {
        devLoaderSource: toDayjsDevLoaderSource(loaderSource),
        hasDeclaration,
        loaderSource,
        moduleSource,
        packageName: manifest.name,
        packageVersion: manifest.version
      });
    }
  }

  return [...inventory.values()].sort((left, right) =>
    left.moduleSource.localeCompare(right.moduleSource)
  );
}

function readPackageManifest(packageRoot: string): {
  name: string;
  version: string;
} {
  const value = JSON.parse(
    readFileSync(path.join(packageRoot, 'package.json'), 'utf8')
  ) as Record<string, unknown>;
  if (
    value.name !== DAYJS_PACKAGE_NAME ||
    typeof value.version !== 'string' ||
    value.version.length === 0
  ) {
    throw new Error(
      `[native-dayjs-modules] Invalid package manifest at '${packageRoot}'.`
    );
  }
  return { name: value.name, version: value.version };
}

function listJavaScriptFiles(directory: string, prefix = ''): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    if (entry.name === 'node_modules') return [];
    const relativePath = `${prefix}${entry.name}`;
    if (entry.isDirectory()) {
      return listJavaScriptFiles(
        path.join(directory, entry.name),
        `${relativePath}/`
      );
    }
    return entry.isFile() &&
      JAVASCRIPT_EXTENSIONS.some((extension) => entry.name.endsWith(extension))
      ? [relativePath]
      : [];
  });
}

function hasAdjacentDeclaration(
  packageRoot: string,
  relativeFile: string
): boolean {
  return JAVASCRIPT_EXTENSIONS.some(
    (extension) =>
      relativeFile.endsWith(extension) &&
      existsSync(
        path.join(
          packageRoot,
          `${relativeFile.slice(0, -extension.length)}.d.ts`
        )
      )
  );
}

function withoutJavaScriptExtension(moduleSource: string): string {
  const extension = JAVASCRIPT_EXTENSIONS.find((candidate) =>
    moduleSource.endsWith(candidate)
  );
  return extension ? moduleSource.slice(0, -extension.length) : moduleSource;
}

function toDayjsDevLoaderSource(loaderSource: string): string {
  if (loaderSource === 'dayjs/dayjs.min.js') return 'dayjs/esm/index.js';
  const plugin = /^dayjs\/plugin\/([^/]+)\.js$/u.exec(loaderSource);
  if (plugin) return `dayjs/esm/plugin/${plugin[1]}/index.js`;
  const locale = /^dayjs\/locale\/([^/]+)\.js$/u.exec(loaderSource);
  if (locale) return `dayjs/esm/locale/${locale[1]}.js`;
  return loaderSource;
}

function renderNativeDayjsModules(
  inventory: readonly DayjsModuleSource[]
): string {
  return generateNativeDayjsModules(inventory, 'build');
}

export function generateNativeDayjsDevModules(
  inventory: readonly DayjsModuleSource[]
): string {
  return generateNativeDayjsModules(inventory, 'serve');
}

function generateNativeDayjsModules(
  inventory: readonly DayjsModuleSource[],
  command: 'build' | 'serve'
): string {
  const packageIdentity = inventory[0];
  if (!packageIdentity) {
    throw new Error('[native-dayjs-modules] Module inventory is empty.');
  }
  const loaderRuntime = generateDemandResolvedLoaderRuntime({
    command,
    errorLabel: 'dayjs module',
    modules: inventory,
    virtualPrefix: NATIVE_DAYJS_LEAF_VIRTUAL_PREFIX
  });

  return `
${loaderRuntime.preamble}

export const DAYJS_MODULE_DEFINITIONS = Object.freeze(
  ${JSON.stringify(loaderRuntime.moduleSources)}.map((module_source) => ({
    module_source,
    exports: module_source === 'dayjs' ? ['default'] : ['*']
  }))
);

export const DAYJS_DECLARATION_SOURCES = Object.freeze(${JSON.stringify(
    inventory
      .filter(({ hasDeclaration }) => hasDeclaration)
      .map(({ moduleSource }) => moduleSource)
  )});

export const DAYJS_PACKAGE = Object.freeze(${JSON.stringify({
    package_name: packageIdentity.packageName,
    package_version: packageIdentity.packageVersion,
    module_count: inventory.length
  })});

export async function loadDayjsModule(moduleSource) {
  ${loaderRuntime.loadBody}
}
`;
}
