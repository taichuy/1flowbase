import { readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';

import type { Plugin } from 'vite';

export const NATIVE_DND_KIT_MODULES_VIRTUAL_ID =
  'virtual:1flowbase-native-dnd-kit-modules';

const RESOLVED_VIRTUAL_ID = `\0${NATIVE_DND_KIT_MODULES_VIRTUAL_ID}`;
const DND_KIT_SCOPE = '@dnd-kit';
const JAVASCRIPT_EXTENSIONS = ['.js', '.mjs', '.cjs'] as const;

export interface DndKitModuleSource {
  loaderSource: string;
  moduleSource: string;
  packageName: string;
  packageVersion: string;
}

export function nativeDndKitModulesPlugin({
  inventory
}: {
  inventory: readonly DndKitModuleSource[];
}): Plugin {
  return {
    name: '1flowbase-native-dnd-kit-modules',
    enforce: 'pre',
    resolveId(id) {
      return id === NATIVE_DND_KIT_MODULES_VIRTUAL_ID
        ? RESOLVED_VIRTUAL_ID
        : undefined;
    },
    load(id) {
      return id === RESOLVED_VIRTUAL_ID
        ? renderNativeDndKitModules(inventory)
        : undefined;
    }
  };
}

export function collectDndKitModuleSources({
  projectRoot
}: {
  projectRoot: string;
}): DndKitModuleSource[] {
  const scopeRoot = path.join(projectRoot, 'node_modules', DND_KIT_SCOPE);
  const inventory = new Map<string, DndKitModuleSource>();

  for (const entry of readdirSync(scopeRoot, { withFileTypes: true }).sort(
    (left, right) => left.name.localeCompare(right.name)
  )) {
    const packageRoot = path.join(scopeRoot, entry.name);
    if (!statSync(packageRoot).isDirectory()) continue;
    const manifest = readPackageManifest(packageRoot);
    const expectedPackageName = `${DND_KIT_SCOPE}/${entry.name}`;
    if (manifest.name !== expectedPackageName) {
      throw new Error(
        `[native-dnd-kit-modules] Expected '${expectedPackageName}', received '${manifest.name}'.`
      );
    }

    const packageEntry = {
      loaderSource: manifest.name,
      moduleSource: manifest.name,
      packageName: manifest.name,
      packageVersion: manifest.version
    };
    inventory.set(packageEntry.moduleSource, packageEntry);

    for (const relativeFile of listJavaScriptFiles(packageRoot)) {
      const loaderSource = `${manifest.name}/${relativeFile}`;
      const aliases = new Set([
        loaderSource,
        withoutJavaScriptExtension(loaderSource)
      ]);
      if (relativeFile.endsWith('/index.js')) {
        aliases.add(loaderSource.slice(0, -'/index.js'.length));
      }
      for (const moduleSource of aliases) {
        inventory.set(moduleSource, {
          loaderSource,
          moduleSource,
          packageName: manifest.name,
          packageVersion: manifest.version
        });
      }
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
    typeof value.name !== 'string' ||
    !value.name.startsWith(`${DND_KIT_SCOPE}/`) ||
    typeof value.version !== 'string' ||
    value.version.length === 0
  ) {
    throw new Error(
      `[native-dnd-kit-modules] Invalid package manifest at '${packageRoot}'.`
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

function withoutJavaScriptExtension(moduleSource: string): string {
  const extension = JAVASCRIPT_EXTENSIONS.find((candidate) =>
    moduleSource.endsWith(candidate)
  );
  return extension ? moduleSource.slice(0, -extension.length) : moduleSource;
}

function renderNativeDndKitModules(
  inventory: readonly DndKitModuleSource[]
): string {
  const packages = [
    ...new Map(
      inventory.map(({ packageName, packageVersion }) => [
        packageName,
        { package_name: packageName, package_version: packageVersion }
      ])
    ).values()
  ];
  return `
const loaders = {${inventory
    .map(
      ({ loaderSource, moduleSource }) =>
        `\n  ${JSON.stringify(moduleSource)}: () => import(${JSON.stringify(loaderSource)}),`
    )
    .join('')}\n};

export const DND_KIT_MODULE_DEFINITIONS = Object.freeze(
  Object.keys(loaders).map((module_source) => ({ module_source, exports: ['*'] }))
);

export const DND_KIT_PACKAGES = Object.freeze(${JSON.stringify(packages)});

export async function loadDndKitModule(moduleSource) {
  const load = loaders[moduleSource];
  if (!load) throw new Error('@dnd-kit module is not installed or resolvable: ' + moduleSource + '.');
  return load();
}
`;
}
