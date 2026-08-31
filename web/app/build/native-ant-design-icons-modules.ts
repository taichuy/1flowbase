import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

import type { Plugin } from 'vite';

export const NATIVE_ANT_DESIGN_ICONS_MODULES_VIRTUAL_ID =
  'virtual:1flowbase-native-ant-design-icons-modules';
export const NATIVE_ANT_DESIGN_ICONS_LOADERS_VIRTUAL_ID =
  'virtual:1flowbase-native-ant-design-icons-loaders';
export const NATIVE_ANT_DESIGN_ICON_LEAF_VIRTUAL_PREFIX =
  'virtual:1flowbase-native-ant-design-icon-leaf/';
export const PRODUCT_ANT_DESIGN_ICON_RESOLVED_IDS = new Set<string>();

const RESOLVED_VIRTUAL_ID = `\0${NATIVE_ANT_DESIGN_ICONS_MODULES_VIRTUAL_ID}`;
const RESOLVED_LOADERS_VIRTUAL_ID = `\0${NATIVE_ANT_DESIGN_ICONS_LOADERS_VIRTUAL_ID}`;
const PACKAGE_NAME = '@ant-design/icons';
const ROOT_EXPORTS = [
  'default',
  'IconProvider',
  'createFromIconfontCN',
  'getTwoToneColor',
  'setTwoToneColor'
] as const;
export const NATIVE_ANT_DESIGN_ICONS_SHARED_MODULE_SOURCES = [
  '@ant-design/icons/es/components/Icon',
  '@ant-design/icons/es/components/IconFont',
  '@ant-design/icons/es/components/Context',
  '@ant-design/icons/es/components/twoTonePrimaryColor'
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
  let command: 'build' | 'serve' = 'build';
  const leafModuleSources = new Map(
    inventory.modules.map(({ moduleSource }) => [
      toLeafVirtualId(moduleSource),
      moduleSource
    ])
  );

  return {
    name: '1flowbase-native-ant-design-icons-modules',
    enforce: 'pre',
    configResolved(config) {
      command = config.command;
    },
    async resolveId(id, importer) {
      if (id === NATIVE_ANT_DESIGN_ICONS_MODULES_VIRTUAL_ID) {
        return RESOLVED_VIRTUAL_ID;
      }
      if (id === NATIVE_ANT_DESIGN_ICONS_LOADERS_VIRTUAL_ID) {
        return RESOLVED_LOADERS_VIRTUAL_ID;
      }
      if (id.startsWith(NATIVE_ANT_DESIGN_ICON_LEAF_VIRTUAL_PREFIX)) {
        if (!leafModuleSources.has(id)) {
          throw new Error(
            '@ant-design/icons module is not installed or public: ' + id + '.'
          );
        }
        return `\0${id}`;
      }
      const productIconName = productAntDesignIconName(id);
      if (
        productIconName &&
        !importer?.includes(NATIVE_ANT_DESIGN_ICON_LEAF_VIRTUAL_PREFIX)
      ) {
        const resolved = await this.resolve(id, importer, { skipSelf: true });
        if (!resolved) return undefined;
        PRODUCT_ANT_DESIGN_ICON_RESOLVED_IDS.add(resolved.id);
        return resolved.id;
      }
      return undefined;
    },
    load(id) {
      if (id === RESOLVED_VIRTUAL_ID) {
        return generateNativeAntDesignIconsModule(inventory);
      }
      if (id === RESOLVED_LOADERS_VIRTUAL_ID) {
        return command === 'serve'
          ? generateNativeAntDesignIconsDevLoadersModule(inventory)
          : generateNativeAntDesignIconsLoadersModule(inventory);
      }
      if (id.startsWith(`\0${NATIVE_ANT_DESIGN_ICON_LEAF_VIRTUAL_PREFIX}`)) {
        const moduleSource = leafModuleSources.get(id.slice(1));
        if (!moduleSource) {
          throw new Error(
            '@ant-design/icons module is not installed or public: ' + id + '.'
          );
        }
        return `export { default } from ${JSON.stringify(moduleSource)};`;
      }
      return undefined;
    }
  };
}

function productAntDesignIconName(id: string): string | null {
  return (
    /^@ant-design\/icons\/es\/icons\/([A-Za-z][A-Za-z0-9]*)$/u.exec(id)?.[1] ??
    null
  );
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
        entry.isFile() &&
        entry.name.endsWith('.js') &&
        entry.name !== 'index.js'
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
    rootExports: [...new Set([...ROOT_EXPORTS, ...iconNames])].sort(
      (left, right) => left.localeCompare(right)
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

function assertPublicLeafExport(
  exportsValue: unknown,
  packageRoot: string
): void {
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
let loaderDomainPromise;

async function loadLoaderDomain() {
  loaderDomainPromise ??= import(${JSON.stringify(NATIVE_ANT_DESIGN_ICONS_LOADERS_VIRTUAL_ID)}).catch((error) => {
    loaderDomainPromise = undefined;
    throw error;
  });
  return loaderDomainPromise;
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
  const loaderDomain = await loadLoaderDomain();
  return loaderDomain.loadAntDesignIconsModuleFromDomain(moduleSource);
}
`;
}

export function generateNativeAntDesignIconsLoadersModule(
  inventory: AntDesignIconModuleInventory
): string {
  return generateNativeAntDesignIconsLoaderDomain(inventory, 'build');
}

export function generateNativeAntDesignIconsDevLoadersModule(
  inventory: AntDesignIconModuleInventory
): string {
  return generateNativeAntDesignIconsLoaderDomain(inventory, 'serve');
}

function generateNativeAntDesignIconsLoaderDomain(
  inventory: AntDesignIconModuleInventory,
  command: 'build' | 'serve'
): string {
  const leafIndex =
    command === 'build'
      ? `const leafLoaders = {${inventory.modules
          .map(
            ({ loaderSource, moduleSource }) =>
              `\n  ${JSON.stringify(moduleSource)}: () => import(${JSON.stringify(toLeafVirtualId(loaderSource))}),`
          )
          .join('')}\n};`
      : `const leafModuleSources = new Set(${JSON.stringify(inventory.modules.map(({ moduleSource }) => moduleSource))});`;
  const leafLoad =
    command === 'build'
      ? `const load = leafLoaders[moduleSource];
  if (!load) throw new Error('@ant-design/icons module is not installed or public: ' + moduleSource + '.');`
      : `if (!leafModuleSources.has(moduleSource)) throw new Error('@ant-design/icons module is not installed or public: ' + moduleSource + '.');
  const leafId = ${JSON.stringify(`/@id/__x00__${NATIVE_ANT_DESIGN_ICON_LEAF_VIRTUAL_PREFIX}`)} + moduleSource.slice('@ant-design/icons/'.length);
  const load = () => import(/* @vite-ignore */ leafId);`;
  const rootLeafSources =
    command === 'build' ? 'Object.keys(leafLoaders)' : '[...leafModuleSources]';

  return `
import { lazy } from 'react';

${leafIndex}

const leafModuleFlights = new Map();

function loadLeafModule(moduleSource) {
  const current = leafModuleFlights.get(moduleSource);
  if (current) return current;
  ${leafLoad}
  const flight = load().catch((error) => {
    if (leafModuleFlights.get(moduleSource) === flight) {
      leafModuleFlights.delete(moduleSource);
    }
    throw error;
  });
  leafModuleFlights.set(moduleSource, flight);
  return flight;
}

let rootModulePromise;

async function loadRootModule() {
  rootModulePromise ??= Promise.all([
    ${NATIVE_ANT_DESIGN_ICONS_SHARED_MODULE_SOURCES.map(
      (moduleSource) => `import(${JSON.stringify(moduleSource)})`
    ).join(',\n    ')}
  ])
    .then(([iconModule, iconFontModule, contextModule, twoToneModule]) =>
      Object.fromEntries([
        ['default', iconModule.default],
        ['createFromIconfontCN', iconFontModule.default],
        ['IconProvider', contextModule.default.Provider],
        ['getTwoToneColor', twoToneModule.getTwoToneColor],
        ['setTwoToneColor', twoToneModule.setTwoToneColor],
        ...${rootLeafSources}.map((moduleSource) => [
          moduleSource.slice('@ant-design/icons/'.length),
          lazy(() => loadLeafModule(moduleSource))
        ])
      ])
    )
    .catch((error) => {
      rootModulePromise = undefined;
      throw error;
    });
  return rootModulePromise;
}

export async function loadAntDesignIconsModuleFromDomain(moduleSource) {
  if (moduleSource === '@ant-design/icons') return loadRootModule();
  return loadLeafModule(moduleSource);
}
`;
}

function toLeafVirtualId(moduleSource: string): string {
  return `${NATIVE_ANT_DESIGN_ICON_LEAF_VIRTUAL_PREFIX}${moduleSource.slice(`${PACKAGE_NAME}/`.length)}`;
}
