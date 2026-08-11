import type { FrontstageBlockCatalogEntry } from './block-catalog';

export interface ExternalNpmModuleAsset {
  role: 'browser_module' | 'shadow_style' | 'support';
  media_type: string;
  sha256: string;
  url: string;
}

export interface ExternalNpmModule {
  source: string;
  version: string;
  binding: 'fetched';
  assets: ExternalNpmModuleAsset[];
  exports: string[];
  type_declarations: string;
}

export async function fetchExternalNpmModules(
  fetchAsset: typeof fetch = globalThis.fetch
): Promise<ExternalNpmModule[]> {
  const response = await fetchAsset('/external-npm/manifest.json', {
    credentials: 'same-origin',
    cache: 'no-cache',
    headers: { Accept: 'application/json' }
  });
  if (response.status === 404) return [];
  if (!response.ok) {
    throw new Error('External npm manifest request failed.');
  }
  return normalizeExternalNpmManifest(await response.json());
}

export function normalizeExternalNpmManifest(
  value: unknown
): ExternalNpmModule[] {
  if (
    !isRecord(value) ||
    value.schema_version !== 1 ||
    !Array.isArray(value.modules)
  ) {
    throw invalidManifest();
  }

  const modules: ExternalNpmModule[] = [];
  const sources = new Set<string>();
  for (const item of value.modules) {
    if (
      !isRecord(item) ||
      !isNonEmptyString(item.source) ||
      !isNonEmptyString(item.version) ||
      item.binding !== 'fetched' ||
      !Array.isArray(item.assets) ||
      !Array.isArray(item.exports) ||
      item.exports.length === 0 ||
      !item.exports.every(isNonEmptyString) ||
      new Set(item.exports).size !== item.exports.length ||
      !isNonEmptyString(item.type_declarations) ||
      sources.has(item.source)
    ) {
      throw invalidManifest();
    }
    const assets = item.assets.map(normalizeAsset);
    if (
      assets.some((asset) => asset === null) ||
      assets.filter((asset) => asset?.role === 'browser_module').length !== 1
    ) {
      throw invalidManifest();
    }
    sources.add(item.source);
    modules.push({
      source: item.source,
      version: item.version,
      binding: 'fetched',
      assets: assets as ExternalNpmModuleAsset[],
      exports: [...item.exports],
      type_declarations: item.type_declarations
    });
  }
  return modules;
}

export function mergeExternalNpmModules(
  entries: FrontstageBlockCatalogEntry[],
  modules: ExternalNpmModule[]
): FrontstageBlockCatalogEntry[] {
  if (modules.length === 0) return entries;
  return entries.map((entry) => {
    const existingSources = new Set(
      entry.code_modules.map((module) => module.source)
    );
    const conflict = modules.find((module) =>
      existingSources.has(module.source)
    );
    if (conflict) {
      throw new Error(
        `External npm module conflicts with the block catalog: ${conflict.source}.`
      );
    }
    return {
      ...entry,
      code_modules: [...entry.code_modules, ...modules]
    };
  });
}

function normalizeAsset(value: unknown): ExternalNpmModuleAsset | null {
  if (
    !isRecord(value) ||
    (value.role !== 'browser_module' &&
      value.role !== 'shadow_style' &&
      value.role !== 'support') ||
    !isNonEmptyString(value.media_type) ||
    !isSha256(value.sha256) ||
    !isExternalAssetUrl(value.url, value.sha256)
  ) {
    return null;
  }
  return {
    role: value.role,
    media_type: value.media_type,
    sha256: value.sha256,
    url: value.url
  };
}

function isExternalAssetUrl(value: unknown, sha256: string): value is string {
  if (typeof value !== 'string' || !value.startsWith('/external-npm/assets/')) {
    return false;
  }
  try {
    const url = new URL(value, 'https://1flowbase.invalid');
    const fileName = url.pathname.split('/').at(-1) ?? '';
    return (
      url.origin === 'https://1flowbase.invalid' &&
      url.pathname === value &&
      url.search === '' &&
      url.hash === '' &&
      new RegExp(`^[A-Za-z0-9._-]+-${sha256}\\.[A-Za-z0-9]+$`, 'u').test(
        fileName
      )
    );
  } catch {
    return false;
  }
}

function invalidManifest() {
  return new Error('External npm manifest is invalid.');
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}

function isSha256(value: unknown): value is string {
  return typeof value === 'string' && /^[a-f0-9]{64}$/u.test(value);
}
