import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

import type { Plugin } from 'vite';

export const NATIVE_ANT_DESIGN_ICONS_MODULES_VIRTUAL_ID =
  'virtual:1flowbase-native-ant-design-icons-modules';

const RESOLVED_VIRTUAL_ID =
  `\0${NATIVE_ANT_DESIGN_ICONS_MODULES_VIRTUAL_ID}`;
const PACKAGE_NAME = '@ant-design/icons';
const ROOT_EXPORTS = [
  'default',
  'IconProvider',
  'createFromIconfontCN',
  'getTwoToneColor',
  'setTwoToneColor'
] as const;

export interface AntDesignIconModuleSource {
  loaderSource: string;
  moduleSource: string;
}

export interface AntDesignIconModuleInventory {
  modules: AntDesignIconModuleSource[];
  packageName: string;
  packageVersion: string;
  rootExports: string[];
}

export function nativeAntDesignIconsModulesPlugin({
  inventory
}: {
  inventory: AntDesignIconModuleInventory;
}): Plugin {
  return {
    name: '1flowbase-native-ant-design-icons-modules',
    enforce: 'pre',
    resolveId(id) {
      return id === NATIVE_ANT_DESIGN_ICONS_MODULES_VIRTUAL_ID
        ? RESOLVED_VIRTUAL_ID
        : undefined;
    },
    load(id) {
      return id === RESOLVED_VIRTUAL_ID
        ? generateNativeAntDesignIconsModule(inventory)
        : undefined;
    }
  };
}

export function collectAntDesignIconModuleSources({
  projectRoot
}: {
  projectRoot: string;
}): AntDesignIconModuleInventory {
  const packageRoot = path.join(projectRoot, 'node_modules', PACKAGE_NAME);
  const manifest = readPackageManifest(packageRoot);
  assertPublicLeafExport(manifest.exports, packageRoot);
  const iconJavaScriptRoot = path.join(packageRoot, 'es/icons');
  const iconDeclarationRoot = path.join(packageRoot, 'lib/icons');
  const iconNames = readdirSync(iconJavaScriptRoot, { withFileTypes: true })
    .filter(
      (entry) =>
        entry.isFile() && entry.name.endsWith('.js') && entry.name !== 'index.js'
    )
    .map((entry) => entry.name.slice(0, -'.js'.length))
    .sort((left, right) => left.localeCompare(right));

  for (const iconName of iconNames) {
    if (!existsSync(path.join(iconDeclarationRoot, `${iconName}.d.ts`))) {
      throw new Error(
        `[native-ant-design-icons-modules] Public icon '${iconName}' has no declaration.`
      );
    }
  }

  return {
    modules: iconNames.map((iconName) => {
      const moduleSource = `${PACKAGE_NAME}/${iconName}`;
      return { loaderSource: moduleSource, moduleSource };
    }),
    packageName: manifest.name,
    packageVersion: manifest.version,
    rootExports: [...new Set([...ROOT_EXPORTS, ...iconNames])].sort((left, right) =>
      left.localeCompare(right)
    )
  };
}

function readPackageManifest(packageRoot: string): {
  exports: unknown;
  name: string;
  version: string;
} {
  const value = JSON.parse(
    readFileSync(path.join(packageRoot, 'package.json'), 'utf8')
  ) as Record<string, unknown>;
  if (
    value.name !== PACKAGE_NAME ||
    typeof value.version !== 'string' ||
    value.version.length === 0
  ) {
    throw new Error(
      `[native-ant-design-icons-modules] Invalid package manifest at '${packageRoot}'.`
    );
  }
  return { exports: value.exports, name: value.name, version: value.version };
}

function assertPublicLeafExport(exportsValue: unknown, packageRoot: string): void {
  if (!exportsValue || typeof exportsValue !== 'object') {
    throw new Error(
      `[native-ant-design-icons-modules] Package at '${packageRoot}' has no exports map.`
    );
  }
  const publicLeaf = (exportsValue as Record<string, unknown>)['./*'];
  if (!publicLeaf || typeof publicLeaf !== 'object') {
    throw new Error(
      `[native-ant-design-icons-modules] Package at '${packageRoot}' has no public icon leaf export.`
    );
  }
  const conditions = publicLeaf as Record<string, unknown>;
  if (
    conditions.import !== './es/icons/*.js' ||
    conditions.types !== './lib/icons/*.d.ts'
  ) {
    throw new Error(
      `[native-ant-design-icons-modules] Unsupported public icon leaf export at '${packageRoot}'.`
    );
  }
}

export function generateNativeAntDesignIconsModule(
  inventory: AntDesignIconModuleInventory
): string {
  return `
import { lazy } from 'react';

const leafLoaders = {${inventory.modules
    .map(
      ({ loaderSource, moduleSource }) =>
        `\n  ${JSON.stringify(moduleSource)}: () => import(${JSON.stringify(loaderSource)}),`
    )
    .join('')}\n};

let rootModulePromise;

async function loadRootModule() {
  rootModulePromise ??= Promise.all([
    import('@ant-design/icons/es/components/Icon'),
    import('@ant-design/icons/es/components/IconFont'),
    import('@ant-design/icons/es/components/Context'),
    import('@ant-design/icons/es/components/twoTonePrimaryColor')
  ]).then(([iconModule, iconFontModule, contextModule, twoToneModule]) =>
    Object.fromEntries([
      ['default', iconModule.default],
      ['createFromIconfontCN', iconFontModule.default],
      ['IconProvider', contextModule.default.Provider],
      ['getTwoToneColor', twoToneModule.getTwoToneColor],
      ['setTwoToneColor', twoToneModule.setTwoToneColor],
      ...Object.entries(leafLoaders).map(([moduleSource, load]) => [
        moduleSource.slice('@ant-design/icons/'.length),
        lazy(load)
      ])
    ])
  );
  return rootModulePromise;
}

export const ANT_DESIGN_ICONS_MODULE_DEFINITIONS = Object.freeze([
  { module_source: ${JSON.stringify(PACKAGE_NAME)}, exports: Object.freeze(${JSON.stringify(inventory.rootExports)}) },
  ...${JSON.stringify(inventory.modules.map(({ moduleSource }) => moduleSource))}.map((module_source) => ({
    module_source,
    exports: Object.freeze(['default'])
  }))
]);

export const ANT_DESIGN_ICONS_PACKAGE = Object.freeze(${JSON.stringify({
    package_name: inventory.packageName,
    package_version: inventory.packageVersion,
    module_count: inventory.modules.length
  })});

export async function loadAntDesignIconsModule(moduleSource) {
  if (moduleSource === '@ant-design/icons') return loadRootModule();
  const load = leafLoaders[moduleSource];
  if (!load) throw new Error('@ant-design/icons module is not installed or public: ' + moduleSource + '.');
  return load();
}
`;
}
