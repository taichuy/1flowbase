import { transform } from 'sucrase';

import { tokenizeSource } from '../../native-trusted-block/source-evaluator-transform';
import type { NativeTrustedBlockInjectedModuleMap } from '../../native-trusted-block/source-evaluator-types';
import {
  canonicalizeNativeReactCatalogDependencyLock,
  nativeReactBrowserModuleAsset,
  nativeReactCatalogModuleIdentity,
  type NativeReactCatalogDependencyLock,
  type NativeReactCatalogModuleLock,
  type NativeReactModuleAssetLock
} from './contracts';

export type NativeReactModuleRegistryErrorCode =
  | 'invalid_dependency_lock'
  | 'module_not_registered'
  | 'module_fetch_failed'
  | 'module_digest_mismatch'
  | 'module_media_type_mismatch'
  | 'module_invalid'
  | 'module_dependency_denied'
  | 'module_dependency_cycle'
  | 'module_export_missing';

export class NativeReactModuleRegistryError extends Error {
  readonly code: NativeReactModuleRegistryErrorCode;
  readonly path: string;

  constructor(
    code: NativeReactModuleRegistryErrorCode,
    path: string,
    message: string
  ) {
    super(message);
    this.name = 'NativeReactModuleRegistryError';
    this.code = code;
    this.path = path;
  }
}

export interface NativeReactModuleRegistryOptions {
  dependencyLock: NativeReactCatalogDependencyLock;
  hostModules: NativeTrustedBlockInjectedModuleMap;
  fetchAsset?: typeof fetch;
  crypto?: Pick<Crypto, 'subtle'>;
}

interface PreparedModule {
  registration: NativeReactCatalogModuleLock;
  code: string;
  dependencies: string[];
}

export interface NativeReactModuleRegistry {
  load(moduleSource: string): Promise<Record<string, unknown>>;
  resolveModuleMap(
    moduleSources: readonly string[]
  ): Promise<NativeTrustedBlockInjectedModuleMap>;
  resolveModuleAssets(
    moduleSources: readonly string[]
  ): Promise<NativeReactResolvedModuleAsset[]>;
}

export interface NativeReactResolvedModuleAsset {
  module_source: string;
  role: 'shadow_style' | 'support';
  media_type: string;
  sha256: string;
  url: string;
  bytes: ArrayBuffer;
}

export function createNativeReactModuleRegistry({
  dependencyLock: dependencyLockValue,
  hostModules,
  fetchAsset = globalThis.fetch,
  crypto = globalThis.crypto
}: NativeReactModuleRegistryOptions): NativeReactModuleRegistry {
  const dependencyLock =
    canonicalizeNativeReactCatalogDependencyLock(dependencyLockValue);
  if (!dependencyLock) {
    throw registryError(
      'invalid_dependency_lock',
      'dependencyLock',
      'Native React Catalog dependency lock is invalid.'
    );
  }
  if (typeof fetchAsset !== 'function' || !crypto?.subtle) {
    throw registryError(
      'module_fetch_failed',
      'moduleRegistry.host',
      'Native React module loading is unavailable.'
    );
  }

  const registrations = new Map(
    dependencyLock.map((entry) => [entry.module_source, entry])
  );
  for (const registration of registrations.values()) {
    if (
      registration.binding === 'host' &&
      !hostModules[registration.module_source]
    ) {
      throw registryError(
        'invalid_dependency_lock',
        `dependencyLock.${registration.module_source}`,
        `Host ABI module is unavailable: ${registration.module_source}.`
      );
    }
    if (
      registration.binding === 'host' &&
      registration.exports.some(
        (exportName) =>
          !(exportName in hostModules[registration.module_source]!)
      )
    ) {
      throw registryError(
        'invalid_dependency_lock',
        `dependencyLock.${registration.module_source}.exports`,
        `Host ABI module exports do not match: ${registration.module_source}.`
      );
    }
    if (
      registration.binding === 'fetched' &&
      !registration.assets.every((asset) =>
        isRegisteredAssetUrl(registration, asset)
      )
    ) {
      throw registryError(
        'invalid_dependency_lock',
        `dependencyLock.${registration.module_source}.assets`,
        `Catalog module asset URL is invalid: ${registration.module_source}.`
      );
    }
  }

  const preparationFlights = new Map<string, Promise<PreparedModule>>();
  const evaluationFlights = new Map<string, Promise<Record<string, unknown>>>();
  const assetFlights = new Map<string, Promise<ArrayBuffer>>();
  const dependencyGraph = new Map<string, Set<string>>();

  const prepare = (registration: NativeReactCatalogModuleLock) => {
    const identity = nativeReactCatalogModuleIdentity(registration);
    let flight = preparationFlights.get(identity);
    if (!flight) {
      flight = fetchAndPrepare(registration, fetchAsset, crypto);
      preparationFlights.set(identity, flight);
    }
    return flight;
  };

  const load = (moduleSource: string): Promise<Record<string, unknown>> => {
    const registration = registrations.get(moduleSource);
    if (!registration) {
      return Promise.reject(
        registryError(
          'module_not_registered',
          `modules.${moduleSource}`,
          `Catalog module is not registered: ${moduleSource}.`
        )
      );
    }
    if (registration.binding === 'host') {
      return validateHostModule(registration, hostModules);
    }
    const identity = nativeReactCatalogModuleIdentity(registration);
    let flight = evaluationFlights.get(identity);
    if (!flight) {
      flight = evaluateRegisteredModule(
        registration,
        prepare,
        load,
        registrations,
        hostModules,
        dependencyGraph
      );
      evaluationFlights.set(identity, flight);
    }
    return flight;
  };

  return {
    load,
    async resolveModuleMap(moduleSources) {
      const moduleMap: NativeTrustedBlockInjectedModuleMap = {
        ...hostModules
      };
      const uniqueSources = [...new Set(moduleSources)].filter(
        (source) => !hostModules[source]
      );
      const namespaces = await Promise.all(uniqueSources.map(load));
      uniqueSources.forEach((source, index) => {
        moduleMap[source] = namespaces[index];
      });
      return moduleMap;
    },
    async resolveModuleAssets(moduleSources) {
      const resolvedSources = new Set<string>();
      const pendingSources = [...new Set(moduleSources)];
      while (pendingSources.length > 0) {
        const source = pendingSources.shift()!;
        if (resolvedSources.has(source)) continue;
        resolvedSources.add(source);
        for (const dependency of dependencyGraph.get(source) ?? []) {
          pendingSources.push(dependency);
        }
      }
      const assets = [...resolvedSources].flatMap((source) => {
        const registration = registrations.get(source);
        if (!registration) {
          throw registryError(
            'module_not_registered',
            `modules.${source}`,
            `Catalog module is not registered: ${source}.`
          );
        }
        return registration.assets
          .filter((asset) => asset.role !== 'browser_module')
          .map((asset) => ({ registration, asset }));
      });
      return Promise.all(
        assets.map(async ({ registration, asset }) => ({
          module_source: registration.module_source,
          role: asset.role as 'shadow_style' | 'support',
          media_type: asset.media_type,
          sha256: asset.sha256,
          url: asset.url,
          bytes: await fetchVerifiedAsset(
            registration,
            asset,
            fetchAsset,
            crypto,
            assetFlights
          )
        }))
      );
    }
  };
}

async function fetchAndPrepare(
  registration: NativeReactCatalogModuleLock,
  fetchAsset: typeof fetch,
  crypto: Pick<Crypto, 'subtle'>
): Promise<PreparedModule> {
  const browserAsset = nativeReactBrowserModuleAsset(registration);
  if (!browserAsset)
    throw new Error('Fetched module browser asset is missing.');
  const bytes = await fetchVerifiedAsset(
    registration,
    browserAsset,
    fetchAsset,
    crypto
  );

  try {
    const source = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    const code = transform(source, {
      transforms: ['imports'],
      filePath: `${registration.module_source}.js`
    }).code;
    return {
      registration,
      code,
      dependencies: collectTransformedDependencies(code)
    };
  } catch {
    throw registryError(
      'module_invalid',
      `modules.${registration.module_source}.assets.browser_module`,
      `Catalog module asset is not a valid ESM module: ${registration.module_source}.`
    );
  }
}

async function evaluateRegisteredModule(
  registration: NativeReactCatalogModuleLock,
  prepare: (
    registration: NativeReactCatalogModuleLock
  ) => Promise<PreparedModule>,
  load: (moduleSource: string) => Promise<Record<string, unknown>>,
  registrations: ReadonlyMap<string, NativeReactCatalogModuleLock>,
  hostModules: NativeTrustedBlockInjectedModuleMap,
  dependencyGraph: Map<string, Set<string>>
): Promise<Record<string, unknown>> {
  const prepared = await prepare(registration);
  dependencyGraph.set(
    registration.module_source,
    new Set(prepared.dependencies.filter((source) => registrations.has(source)))
  );
  if (hasDependencyCycle(registration.module_source, dependencyGraph)) {
    throw registryError(
      'module_dependency_cycle',
      `modules.${registration.module_source}.imports`,
      `Catalog module dependency cycle is not allowed: ${registration.module_source}.`
    );
  }
  const dependencyNamespaces = new Map<string, Record<string, unknown>>();
  await Promise.all(
    prepared.dependencies.map(async (source) => {
      const hostModule = hostModules[source];
      if (hostModule) {
        const hostRegistration =
          registrations.get(source) ??
          (source === 'react/jsx-runtime'
            ? registrations.get('react')
            : undefined);
        if (hostRegistration?.binding !== 'host') {
          throw registryError(
            'module_dependency_denied',
            `modules.${registration.module_source}.imports.${source}`,
            `Host module dependency is not registered: ${source}.`
          );
        }
        dependencyNamespaces.set(source, hostModule);
        return;
      }
      if (!registrations.has(source)) {
        throw registryError(
          'module_dependency_denied',
          `modules.${registration.module_source}.imports.${source}`,
          `Catalog module dependency is not registered: ${source}.`
        );
      }
      dependencyNamespaces.set(source, await load(source));
    })
  );

  const exports: Record<string, unknown> = {};
  const module = { exports };
  const requireRegistered = (source: string): Record<string, unknown> => {
    const dependency = dependencyNamespaces.get(source);
    if (!dependency) {
      throw registryError(
        'module_dependency_denied',
        `modules.${registration.module_source}.imports.${source}`,
        `Catalog module dependency is not registered: ${source}.`
      );
    }
    return dependency;
  };

  try {
    const evaluator = new Function(
      'require',
      'exports',
      'module',
      `"use strict";\n${prepared.code}\nreturn module.exports;`
    ) as (
      require: (source: string) => Record<string, unknown>,
      exports: Record<string, unknown>,
      module: { exports: Record<string, unknown> }
    ) => unknown;
    const evaluated = evaluator(requireRegistered, exports, module);
    if (!isRecord(evaluated)) throw new Error('Module namespace is invalid.');
    for (const exportName of registration.exports) {
      if (!(exportName in evaluated)) {
        throw registryError(
          'module_export_missing',
          `modules.${registration.module_source}.exports.${exportName}`,
          `Catalog module export is missing: ${registration.module_source}.${exportName}.`
        );
      }
    }
    return evaluated;
  } catch (error) {
    if (error instanceof NativeReactModuleRegistryError) throw error;
    throw registryError(
      'module_invalid',
      `modules.${registration.module_source}.evaluate`,
      `Catalog module evaluation failed: ${registration.module_source}.`
    );
  }
}

function hasDependencyCycle(
  start: string,
  graph: ReadonlyMap<string, ReadonlySet<string>>
): boolean {
  const visiting = new Set<string>();
  const visited = new Set<string>();
  const visit = (source: string): boolean => {
    if (visiting.has(source)) return true;
    if (visited.has(source)) return false;
    visiting.add(source);
    for (const dependency of graph.get(source) ?? []) {
      if (visit(dependency)) return true;
    }
    visiting.delete(source);
    visited.add(source);
    return false;
  };
  return visit(start);
}

function collectTransformedDependencies(source: string): string[] {
  const dependencies = new Set<string>();
  const tokens = tokenizeSource(source);
  for (const token of tokens) {
    if (token.value === 'import') {
      throw new Error('Dynamic import is not allowed.');
    }
    if (token.value !== 'require') continue;
    const dependency = readGeneratedRequire(source, token.end);
    if (!dependency) throw new Error('Transformed require is invalid.');
    dependencies.add(dependency);
  }
  return [...dependencies];
}

function readGeneratedRequire(source: string, start: number): string | null {
  let index = skipWhitespace(source, start);
  if (source[index] !== '(') return null;
  index = skipWhitespace(source, index + 1);
  const quote = source[index];
  if (quote !== '"' && quote !== "'") return null;
  let value = '';
  for (index += 1; index < source.length; index += 1) {
    const char = source[index];
    if (char === '\\') {
      const escaped = source[index + 1];
      if (escaped !== quote && escaped !== '\\') return null;
      value += escaped;
      index += 1;
      continue;
    }
    if (char === quote) {
      index = skipWhitespace(source, index + 1);
      return source[index] === ')' && value.length > 0 ? value : null;
    }
    value += char;
  }
  return null;
}

function skipWhitespace(source: string, start: number): number {
  let index = start;
  while (/\s/.test(source[index] ?? '')) index += 1;
  return index;
}

async function sha256Hex(
  bytes: ArrayBuffer,
  crypto: Pick<Crypto, 'subtle'>
): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
}

function isRegisteredAssetUrl(
  registration: NativeReactCatalogModuleLock,
  asset: NativeReactModuleAssetLock
): boolean {
  const value = asset.url;
  if (!value.startsWith('/') || value.startsWith('//')) return false;
  try {
    const url = new URL(value, 'https://1flowbase.invalid');
    const segments = url.pathname.split('/').filter(Boolean);
    return (
      url.origin === 'https://1flowbase.invalid' &&
      url.search.length === 0 &&
      url.hash.length === 0 &&
      segments.length === 6 &&
      segments[0] === 'api' &&
      segments[1] === 'console' &&
      segments[2] === 'frontstage' &&
      segments[3]!.length > 0 &&
      segments[4] === 'component-module-assets' &&
      segments[5] === asset.sha256
    );
  } catch {
    return false;
  }
}

async function validateHostModule(
  registration: NativeReactCatalogModuleLock,
  hostModules: NativeTrustedBlockInjectedModuleMap
): Promise<Record<string, unknown>> {
  const namespace = hostModules[registration.module_source];
  if (!namespace) {
    throw registryError(
      'module_not_registered',
      `modules.${registration.module_source}`,
      `Host ABI module is unavailable: ${registration.module_source}.`
    );
  }
  for (const exportName of registration.exports) {
    if (!(exportName in namespace)) {
      throw registryError(
        'module_export_missing',
        `modules.${registration.module_source}.exports.${exportName}`,
        `Host ABI export is missing: ${registration.module_source}.${exportName}.`
      );
    }
  }
  return namespace;
}

async function fetchVerifiedAsset(
  registration: NativeReactCatalogModuleLock,
  asset: NativeReactModuleAssetLock,
  fetchAsset: typeof fetch,
  crypto: Pick<Crypto, 'subtle'>,
  flights?: Map<string, Promise<ArrayBuffer>>
): Promise<ArrayBuffer> {
  const identity = `${registration.module_source}:${asset.role}:${asset.sha256}`;
  const existing = flights?.get(identity);
  if (existing) return existing;
  const flight = (async () => {
    let response: Response;
    try {
      response = await fetchAsset(asset.url, { credentials: 'same-origin' });
    } catch {
      throw registryError(
        'module_fetch_failed',
        `modules.${registration.module_source}.assets.${asset.role}`,
        `Catalog module asset request failed: ${registration.module_source}.`
      );
    }
    if (!response.ok) {
      throw registryError(
        'module_fetch_failed',
        `modules.${registration.module_source}.assets.${asset.role}`,
        `Catalog module asset request failed: ${registration.module_source}.`
      );
    }
    if (
      normalizeMediaType(response.headers.get('content-type')) !==
      normalizeMediaType(asset.media_type)
    ) {
      throw registryError(
        'module_media_type_mismatch',
        `modules.${registration.module_source}.assets.${asset.role}.media_type`,
        `Catalog module asset media type mismatch: ${registration.module_source}.`
      );
    }
    let bytes: ArrayBuffer;
    try {
      bytes = await response.arrayBuffer();
    } catch {
      throw registryError(
        'module_fetch_failed',
        `modules.${registration.module_source}.assets.${asset.role}`,
        `Catalog module asset response failed: ${registration.module_source}.`
      );
    }
    if ((await sha256Hex(bytes, crypto)) !== asset.sha256) {
      throw registryError(
        'module_digest_mismatch',
        `modules.${registration.module_source}.assets.${asset.role}.sha256`,
        `Catalog module asset digest mismatch: ${registration.module_source}.`
      );
    }
    return bytes;
  })();
  flights?.set(identity, flight);
  return flight;
}

function normalizeMediaType(value: string | null): string {
  return (value ?? '')
    .split(';')
    .map((part) => part.trim().toLowerCase())
    .filter(Boolean)
    .sort()
    .join(';');
}

function registryError(
  code: NativeReactModuleRegistryErrorCode,
  path: string,
  message: string
): NativeReactModuleRegistryError {
  return new NativeReactModuleRegistryError(code, path, message);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
