import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

import type { Plugin } from 'vite';

export const NATIVE_DAYJS_MODULES_VIRTUAL_ID =
  'virtual:1flowbase-native-dayjs-modules';

const RESOLVED_VIRTUAL_ID = `\0${NATIVE_DAYJS_MODULES_VIRTUAL_ID}`;
const DAYJS_PACKAGE_NAME = 'dayjs';
const JAVASCRIPT_EXTENSIONS = ['.js', '.mjs', '.cjs'] as const;

export interface DayjsModuleSource {
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
  return {
    name: '1flowbase-native-dayjs-modules',
    enforce: 'pre',
    resolveId(id) {
      return id === NATIVE_DAYJS_MODULES_VIRTUAL_ID
        ? RESOLVED_VIRTUAL_ID
        : undefined;
    },
    load(id) {
      return id === RESOLVED_VIRTUAL_ID
        ? renderNativeDayjsModules(inventory)
        : undefined;
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

function renderNativeDayjsModules(
  inventory: readonly DayjsModuleSource[]
): string {
  const packageIdentity = inventory[0];
  if (!packageIdentity) {
    throw new Error('[native-dayjs-modules] Module inventory is empty.');
  }
  return `
const loaders = {${inventory
    .map(
      ({ loaderSource, moduleSource }) =>
        `\n  ${JSON.stringify(moduleSource)}: () => import(${JSON.stringify(loaderSource)}),`
    )
    .join('')}\n};

export const DAYJS_MODULE_DEFINITIONS = Object.freeze(
  Object.keys(loaders).map((module_source) => ({
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
  const load = loaders[moduleSource];
  if (!load) throw new Error('dayjs module is not installed or resolvable: ' + moduleSource + '.');
  return load();
}
`;
}
