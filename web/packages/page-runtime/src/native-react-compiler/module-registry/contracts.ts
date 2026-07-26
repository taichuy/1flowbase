export interface NativeReactBrowserAssetLock {
  sha256: string;
  url: string;
}

export interface NativeReactCatalogModuleLock {
  module_source: string;
  module_version: string;
  browser_asset: NativeReactBrowserAssetLock;
  exports: string[];
}

export type NativeReactCatalogDependencyLock = NativeReactCatalogModuleLock[];

const NATIVE_REACT_HOST_ABI_MODULE_SOURCES = new Set([
  'react',
  'react/jsx-runtime',
  'antd',
  '@1flowbase/ui'
]);

export function canonicalizeNativeReactCatalogDependencyLock(
  value: unknown
): NativeReactCatalogDependencyLock | null {
  if (!Array.isArray(value)) return null;
  const seenSources = new Set<string>();
  const entries: NativeReactCatalogModuleLock[] = [];

  for (const item of value) {
    if (!isRecord(item) || !isRecord(item.browser_asset)) return null;
    const moduleSource = item.module_source;
    const moduleVersion = item.module_version;
    const sha256 = item.browser_asset.sha256;
    const url = item.browser_asset.url;
    if (
      !isNonEmptyString(moduleSource) ||
      !isNonEmptyString(moduleVersion) ||
      !isSha256(sha256) ||
      !isNonEmptyString(url) ||
      !Array.isArray(item.exports) ||
      item.exports.length === 0 ||
      !item.exports.every(isNonEmptyString) ||
      new Set(item.exports).size !== item.exports.length ||
      NATIVE_REACT_HOST_ABI_MODULE_SOURCES.has(moduleSource) ||
      seenSources.has(moduleSource)
    ) {
      return null;
    }
    seenSources.add(moduleSource);
    entries.push({
      module_source: moduleSource,
      module_version: moduleVersion,
      browser_asset: { sha256, url },
      exports: [...item.exports]
    });
  }

  return entries;
}

export function nativeReactCatalogModuleIdentity(
  module: NativeReactCatalogModuleLock
): string {
  return `${module.module_source}@${module.module_version}#${module.browser_asset.sha256}`;
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
      .map((entry) => ({
        module_source: entry.module_source,
        module_version: entry.module_version,
        browser_asset_sha256: entry.browser_asset.sha256,
        exports: [...entry.exports].sort()
      }))
  );
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
