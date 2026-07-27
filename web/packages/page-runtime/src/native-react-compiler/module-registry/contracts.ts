export type NativeReactModuleBinding = 'host' | 'fetched';
export type NativeReactModuleAssetRole =
  | 'browser_module'
  | 'shadow_style'
  | 'support';

export interface NativeReactModuleAssetLock {
  role: NativeReactModuleAssetRole;
  media_type: string;
  sha256: string;
  url: string;
}

export interface NativeReactCatalogModuleLock {
  module_source: string;
  module_version: string;
  binding: NativeReactModuleBinding;
  assets: NativeReactModuleAssetLock[];
  exports: string[];
}

export type NativeReactCatalogDependencyLock = NativeReactCatalogModuleLock[];

const HOST_ABI_MODULE_SOURCES = new Set(['react', 'react/jsx-runtime', 'antd']);

export function canonicalizeNativeReactCatalogDependencyLock(
  value: unknown
): NativeReactCatalogDependencyLock | null {
  if (!Array.isArray(value)) return null;
  const seenSources = new Set<string>();
  const entries: NativeReactCatalogModuleLock[] = [];

  for (const item of value) {
    if (!isRecord(item)) return null;
    const moduleSource = item.module_source;
    const moduleVersion = item.module_version;
    const binding = item.binding;
    if (
      !isNonEmptyString(moduleSource) ||
      !isNonEmptyString(moduleVersion) ||
      (binding !== 'host' && binding !== 'fetched') ||
      !Array.isArray(item.assets) ||
      !Array.isArray(item.exports) ||
      item.exports.length === 0 ||
      !item.exports.every(isNonEmptyString) ||
      new Set(item.exports).size !== item.exports.length ||
      seenSources.has(moduleSource) ||
      (binding === 'host') !== HOST_ABI_MODULE_SOURCES.has(moduleSource)
    ) {
      return null;
    }

    const assets = readAssets(item.assets);
    if (!assets || !hasValidAssetShape(binding, assets)) return null;
    seenSources.add(moduleSource);
    entries.push({
      module_source: moduleSource,
      module_version: moduleVersion,
      binding,
      assets,
      exports: [...item.exports]
    });
  }

  return entries;
}

export function nativeReactCatalogModuleIdentity(
  module: NativeReactCatalogModuleLock
): string {
  return JSON.stringify(canonicalIdentityEntry(module));
}

/** Stable, order-independent input for the Artifact V2 dependency fingerprint. */
export function nativeReactCatalogDependencyLockIdentity(
  dependencyLock: NativeReactCatalogDependencyLock
): string {
  return JSON.stringify(
    [...dependencyLock]
      .sort((left, right) =>
        left.module_source.localeCompare(right.module_source)
      )
      .map(canonicalIdentityEntry)
  );
}

export function nativeReactHostAbiIdentity(
  dependencyLock: NativeReactCatalogDependencyLock
): string {
  return nativeReactCatalogDependencyLockIdentity(
    dependencyLock.filter((entry) => entry.binding === 'host')
  );
}

export function nativeReactBrowserModuleAsset(
  module: NativeReactCatalogModuleLock
): NativeReactModuleAssetLock | null {
  return module.assets.find((asset) => asset.role === 'browser_module') ?? null;
}

function canonicalIdentityEntry(module: NativeReactCatalogModuleLock) {
  return {
    module_source: module.module_source,
    module_version: module.module_version,
    binding: module.binding,
    exports: [...module.exports].sort(),
    assets: [...module.assets]
      .sort((left, right) =>
        `${left.role}:${left.sha256}`.localeCompare(
          `${right.role}:${right.sha256}`
        )
      )
      .map((asset) => ({
        role: asset.role,
        media_type: asset.media_type,
        sha256: asset.sha256
      }))
  };
}

function readAssets(value: unknown[]): NativeReactModuleAssetLock[] | null {
  const assets: NativeReactModuleAssetLock[] = [];
  const identities = new Set<string>();
  for (const item of value) {
    if (!isRecord(item)) return null;
    const { role, media_type: mediaType, sha256, url } = item;
    if (
      (role !== 'browser_module' &&
        role !== 'shadow_style' &&
        role !== 'support') ||
      !isNonEmptyString(mediaType) ||
      !isSha256(sha256) ||
      !isNonEmptyString(url)
    ) {
      return null;
    }
    const identity = `${role}:${sha256}`;
    if (identities.has(identity)) return null;
    identities.add(identity);
    assets.push({ role, media_type: mediaType, sha256, url });
  }
  return assets;
}

function hasValidAssetShape(
  binding: NativeReactModuleBinding,
  assets: NativeReactModuleAssetLock[]
): boolean {
  const browserModules = assets.filter(
    (asset) => asset.role === 'browser_module'
  ).length;
  return binding === 'host'
    ? assets.length === 0
    : browserModules === 1 && assets.length > 0;
}

function isSha256(value: unknown): value is string {
  return typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}
