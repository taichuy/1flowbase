import { createHash } from 'node:crypto';
import {
  existsSync,
  readFileSync,
  readdirSync,
  realpathSync,
  statSync
} from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';

import type { Plugin, ViteDevServer } from 'vite';

const PAGE_TREE_ICON_PREVIEW_ID = 'virtual:1flowbase-page-tree-icon-previews';
const PAGE_TREE_ICON_RUNTIME_ID = 'virtual:1flowbase-page-tree-icon-runtime';
const RESOLVED_PREVIEW_ID = `\0${PAGE_TREE_ICON_PREVIEW_ID}`;
const RESOLVED_RUNTIME_ID = `\0${PAGE_TREE_ICON_RUNTIME_ID}`;
const PREVIEW_PATH_PREFIX = '/__1flowbase_page_tree_icon_pack/';
const ICON_THEME_PATTERN = /(Outlined|Filled|TwoTone)$/u;
const DEFAULT_MAX_PACK_SOURCE_BYTES = 32 * 1024;
const PRIMARY_PREVIEW_COLOR = '#1677ff';
const SECONDARY_PREVIEW_COLOR = '#e6f4ff';

type AbstractIconNode = {
  attrs?: Record<string, boolean | number | string>;
  children?: AbstractIconNode[];
  tag: string;
};

type IconDefinition = {
  icon:
    | AbstractIconNode
    | ((primaryColor: string, secondaryColor: string) => AbstractIconNode);
  name: string;
  theme: string;
};

type IconInventoryEntry = {
  baseName: string;
  componentSource: string;
  definitionPath: string;
  name: string;
  sourceBytes: number;
};

type PageTreeIconPack = {
  id: string;
  icons: IconInventoryEntry[];
  sourceBytes: number;
};

type PageTreeIconInventory = {
  iconNames: string[];
  iconsSvgVersion: string;
  packageVersion: string;
  packs: PageTreeIconPack[];
};

type PageTreeIconPackInput = {
  baseName: string;
  name: string;
  sourceBytes: number;
};

function stablePackId(iconNames: string[]) {
  return `pack-${createHash('sha256')
    .update(iconNames.join('\0'))
    .digest('hex')
    .slice(0, 12)}`;
}

function planPageTreeIconPacks<T extends PageTreeIconPackInput>(
  icons: T[],
  maxSourceBytes = DEFAULT_MAX_PACK_SOURCE_BYTES
): Array<{ id: string; icons: T[]; sourceBytes: number }> {
  if (!Number.isInteger(maxSourceBytes) || maxSourceBytes <= 0) {
    throw new Error('Page tree icon pack budget must be a positive integer');
  }

  const families = new Map<string, T[]>();
  for (const icon of [...icons].sort((left, right) =>
    left.name.localeCompare(right.name)
  )) {
    const family = families.get(icon.baseName) ?? [];
    family.push(icon);
    families.set(icon.baseName, family);
  }

  const packs: T[][] = [];
  let current: T[] = [];
  let currentBytes = 0;
  for (const family of [...families.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([, entries]) => entries)) {
    const familyBytes = family.reduce(
      (total, icon) => total + icon.sourceBytes,
      0
    );
    if (current.length > 0 && currentBytes + familyBytes > maxSourceBytes) {
      packs.push(current);
      current = [];
      currentBytes = 0;
    }
    current.push(...family);
    currentBytes += familyBytes;
  }
  if (current.length > 0) packs.push(current);

  return packs.map((entries) => ({
    id: stablePackId(entries.map(({ name }) => name)),
    icons: entries,
    sourceBytes: entries.reduce((total, icon) => total + icon.sourceBytes, 0)
  }));
}

function collectPageTreeIconInventory({
  projectRoot,
  maxPackSourceBytes = DEFAULT_MAX_PACK_SOURCE_BYTES
}: {
  projectRoot: string;
  maxPackSourceBytes?: number;
}): PageTreeIconInventory {
  const iconPackageRoot = realpathSync(
    path.join(projectRoot, 'node_modules', '@ant-design', 'icons')
  );
  const iconsSvgRoot = resolveIconsSvgRoot(projectRoot, iconPackageRoot);
  const iconManifest = readJson(path.join(iconPackageRoot, 'package.json')) as {
    version?: unknown;
  };
  const iconsSvgManifest = readJson(
    path.join(iconsSvgRoot, 'package.json')
  ) as { version?: unknown };
  if (
    typeof iconManifest.version !== 'string' ||
    typeof iconsSvgManifest.version !== 'string'
  ) {
    throw new Error('Invalid Ant Design icon package identity');
  }

  const definitionsRoot = path.join(iconsSvgRoot, 'lib', 'asn');
  const iconNames = readdirSync(path.join(iconPackageRoot, 'es', 'icons'), {
    withFileTypes: true
  })
    .filter(
      (entry) =>
        entry.isFile() &&
        entry.name.endsWith('.js') &&
        ICON_THEME_PATTERN.test(entry.name.slice(0, -3))
    )
    .map((entry) => entry.name.slice(0, -3))
    .sort((left, right) => left.localeCompare(right));
  const icons = iconNames.map((name) => {
    const definitionPath = path.join(definitionsRoot, `${name}.js`);
    if (!existsSync(definitionPath)) {
      throw new Error(`Missing Ant Design icon definition for '${name}'`);
    }
    return {
      baseName: name.replace(ICON_THEME_PATTERN, ''),
      componentSource: `@ant-design/icons/${name}`,
      definitionPath,
      name,
      sourceBytes: statSync(definitionPath).size
    };
  });

  return {
    iconNames,
    iconsSvgVersion: iconsSvgManifest.version,
    packageVersion: iconManifest.version,
    packs: planPageTreeIconPacks(icons, maxPackSourceBytes)
  };
}

function resolveIconsSvgRoot(projectRoot: string, iconPackageRoot: string) {
  const direct = path.join(
    projectRoot,
    'node_modules',
    '@ant-design',
    'icons-svg'
  );
  if (existsSync(direct)) return realpathSync(direct);

  const packageSibling = path.join(path.dirname(iconPackageRoot), 'icons-svg');
  if (existsSync(packageSibling)) return realpathSync(packageSibling);
  throw new Error(
    '@ant-design/icons-svg must be declared as a direct build dependency'
  );
}

function readJson(filePath: string): unknown {
  return JSON.parse(readFileSync(filePath, 'utf8'));
}

function escapeXml(value: boolean | number | string) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('"', '&quot;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;');
}

function serializeNode(node: AbstractIconNode): string {
  const attributes = Object.entries(node.attrs ?? {})
    .filter(([name]) => name !== 'focusable' && name !== 'viewBox')
    .map(([name, value]) => {
      const normalizedName = name === 'className' ? 'class' : name;
      const normalizedValue =
        value === PRIMARY_PREVIEW_COLOR
          ? 'currentColor'
          : value === SECONDARY_PREVIEW_COLOR
            ? 'currentColor'
            : value;
      const secondaryOpacity =
        value === SECONDARY_PREVIEW_COLOR && name === 'fill'
          ? ' fill-opacity="0.28"'
          : '';
      return ` ${normalizedName}="${escapeXml(normalizedValue)}"${secondaryOpacity}`;
    })
    .join('');
  const children = (node.children ?? []).map(serializeNode).join('');
  return `<${node.tag}${attributes}>${children}</${node.tag}>`;
}

function createPreviewSprite(pack: PageTreeIconPack): string {
  const require = createRequire(import.meta.url);
  const symbols = pack.icons.map(({ definitionPath, name }) => {
    const definition = (require(definitionPath) as { default?: IconDefinition })
      .default;
    if (!definition) {
      throw new Error(`Invalid Ant Design icon definition for '${name}'`);
    }
    const icon =
      typeof definition.icon === 'function'
        ? definition.icon(PRIMARY_PREVIEW_COLOR, SECONDARY_PREVIEW_COLOR)
        : definition.icon;
    const viewBox = icon.attrs?.viewBox ?? '64 64 896 896';
    return `<symbol id="icon-${name}" viewBox="${escapeXml(viewBox)}">${(
      icon.children ?? []
    )
      .map(serializeNode)
      .join('')}</symbol>`;
  });
  return `<svg xmlns="http://www.w3.org/2000/svg"><defs>${symbols.join('')}</defs></svg>`;
}

function generatePreviewModule(
  inventory: PageTreeIconInventory,
  packUrls: Map<string, string>
) {
  const nameToPack = Object.fromEntries(
    inventory.packs.flatMap((pack) =>
      pack.icons.map(({ name }) => [name, pack.id])
    )
  );
  const urls = inventory.packs
    .map(
      ({ id }) =>
        `${JSON.stringify(id)}: ${packUrls.get(id) ?? JSON.stringify(`${PREVIEW_PATH_PREFIX}${id}.svg`)}`
    )
    .join(',\n');
  return `
export const pageTreeIconNames = Object.freeze(${JSON.stringify(inventory.iconNames)});
export const pageTreeIconPackManifest = Object.freeze(${JSON.stringify(
    inventory.packs.map(({ id, icons, sourceBytes }) => ({
      id,
      iconCount: icons.length,
      sourceBytes
    }))
  )});
const nameToPack = Object.freeze(${JSON.stringify(nameToPack)});
const packUrls = Object.freeze({${urls}});
export function pageTreeIconPreviewHref(name) {
  const packId = nameToPack[name];
  return packId ? packUrls[packId] + '#icon-' + name : null;
}
`;
}

function generateRuntimeModule(inventory: PageTreeIconInventory) {
  const loaders = inventory.packs
    .flatMap((pack) => pack.icons)
    .map(
      ({ componentSource, name }) =>
        `${JSON.stringify(name)}: () => import(${JSON.stringify(componentSource)}).then((module) => module.default)`
    )
    .join(',\n');
  return singleFlightRuntimeSource(
    `const iconLoaders = {${loaders}};`,
    `return Boolean(iconLoaders[name]);`,
    `const load = iconLoaders[name];
  if (!load) return null;
  return load();`
  );
}

function singleFlightRuntimeSource(
  domain: string,
  containsBody: string,
  loadBody: string
) {
  return `
${domain}
const iconFlights = new Map();
export function hasPageTreeIconComponent(name) {
  ${containsBody}
}
export function loadPageTreeIconComponent(name) {
  const current = iconFlights.get(name);
  if (current) return current;
  const flight = Promise.resolve().then(async () => {
    ${loadBody}
  }).catch((error) => {
    if (iconFlights.get(name) === flight) iconFlights.delete(name);
    throw error;
  });
  iconFlights.set(name, flight);
  return flight;
}
`;
}

function attachPreviewMiddleware(
  server: ViteDevServer,
  inventory: PageTreeIconInventory
) {
  const packs = new Map(inventory.packs.map((pack) => [pack.id, pack]));
  const sources = new Map<string, string>();
  server.middlewares.use((request, response, next) => {
    const pathname = new URL(request.url ?? '/', 'http://localhost').pathname;
    if (!pathname.startsWith(PREVIEW_PATH_PREFIX)) return next();
    const packId = pathname
      .slice(PREVIEW_PATH_PREFIX.length)
      .replace(/\.svg$/u, '');
    const pack = packs.get(packId);
    if (!pack) {
      response.statusCode = 404;
      response.end('Not found');
      return;
    }
    const source = sources.get(packId) ?? createPreviewSprite(pack);
    sources.set(packId, source);
    response.statusCode = 200;
    response.setHeader('content-type', 'image/svg+xml; charset=utf-8');
    response.setHeader('cache-control', 'no-cache');
    response.end(source);
  });
}

function pageTreeIconAssetsPlugin({
  projectRoot,
  maxPackSourceBytes = DEFAULT_MAX_PACK_SOURCE_BYTES
}: {
  projectRoot: string;
  maxPackSourceBytes?: number;
}): Plugin {
  const inventory = collectPageTreeIconInventory({
    projectRoot,
    maxPackSourceBytes
  });
  const emittedPackRefs = new Map<string, string>();
  let command: 'build' | 'serve' = 'build';

  return {
    name: '1flowbase-page-tree-icon-assets',
    enforce: 'pre',
    configResolved(config) {
      command = config.command;
    },
    buildStart() {
      if (command !== 'build') return;
      for (const pack of inventory.packs) {
        emittedPackRefs.set(
          pack.id,
          this.emitFile({
            type: 'asset',
            name: `page-tree-icons-${pack.id}.svg`,
            source: createPreviewSprite(pack)
          })
        );
      }
    },
    resolveId(id) {
      if (id === PAGE_TREE_ICON_PREVIEW_ID) return RESOLVED_PREVIEW_ID;
      if (id === PAGE_TREE_ICON_RUNTIME_ID) return RESOLVED_RUNTIME_ID;
      return undefined;
    },
    load(id) {
      if (id === RESOLVED_PREVIEW_ID) {
        const urls = new Map(
          inventory.packs.map(({ id: packId }) => {
            const referenceId = emittedPackRefs.get(packId);
            return [
              packId,
              command === 'build' && referenceId
                ? `import.meta.ROLLUP_FILE_URL_${referenceId}`
                : JSON.stringify(`${PREVIEW_PATH_PREFIX}${packId}.svg`)
            ];
          })
        );
        return generatePreviewModule(inventory, urls);
      }
      if (id === RESOLVED_RUNTIME_ID) {
        return generateRuntimeModule(inventory);
      }
      return undefined;
    },
    configureServer(server) {
      attachPreviewMiddleware(server, inventory);
    }
  };
}

export {
  DEFAULT_MAX_PACK_SOURCE_BYTES,
  PAGE_TREE_ICON_PREVIEW_ID,
  PAGE_TREE_ICON_RUNTIME_ID,
  collectPageTreeIconInventory,
  pageTreeIconAssetsPlugin,
  planPageTreeIconPacks
};
export type { PageTreeIconInventory, PageTreeIconPack };
