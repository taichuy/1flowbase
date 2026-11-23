import { sha256Text } from '../../sha256';
import type {
  NativeTrustedBlockInjectedModule,
  NativeTrustedBlockInjectedModuleMap
} from '../../native-trusted-block/source-evaluator-types';
import {
  canonicalizeNativeReactModuleDefinitions,
  type NativeReactModuleDefinition
} from './contracts';

export type NativeReactModuleRegistryErrorCode =
  | 'invalid_module_registry'
  | 'module_not_registered'
  | 'module_load_failed'
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

export interface NativeReactFrontendModuleStyle {
  css: string;
  media_type?: 'text/css' | 'text/css; charset=utf-8';
}

export interface NativeReactFrontendModuleLoadResult {
  module: Record<string, unknown>;
  styles?: readonly NativeReactFrontendModuleStyle[];
}

export interface NativeReactFrontendModuleRegistration extends NativeReactModuleDefinition {
  load(): Promise<NativeReactFrontendModuleLoadResult>;
  loadExports?(
    requestedExports: readonly string[]
  ): Promise<NativeReactFrontendModuleLoadResult>;
}

export interface NativeReactModuleRegistry {
  definitions: readonly NativeReactModuleDefinition[];
  load(moduleSource: string): Promise<Record<string, unknown>>;
  resolveModuleMap(
    moduleSources: readonly string[]
  ): Promise<NativeTrustedBlockInjectedModuleMap>;
  resolveModuleAssets(
    moduleSources: readonly string[]
  ): Promise<NativeReactResolvedModuleAsset[]>;
  resolveInjectedModuleMap?(
    injectedModules: readonly NativeTrustedBlockInjectedModule[]
  ): Promise<NativeTrustedBlockInjectedModuleMap>;
  resolveInjectedModuleAssets?(
    injectedModules: readonly NativeTrustedBlockInjectedModule[]
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

export function createNativeReactModuleRegistry(
  registrationsValue: readonly NativeReactFrontendModuleRegistration[]
): NativeReactModuleRegistry {
  const definitions = canonicalizeNativeReactModuleDefinitions(
    registrationsValue
  );
  if (!definitions || definitions.length !== registrationsValue.length) {
    throw registryError(
      'invalid_module_registry',
      'moduleRegistry',
      'Native React frontend module registry is invalid.'
    );
  }
  const registrations = new Map(
    registrationsValue.map((registration) => [
      registration.module_source,
      registration
    ])
  );
  if (
    registrations.size !== registrationsValue.length ||
    registrationsValue.some(
      (registration) => typeof registration.load !== 'function'
    )
  ) {
    throw registryError(
      'invalid_module_registry',
      'moduleRegistry',
      'Native React frontend module registry registrations are invalid.'
    );
  }

  const flights = new Map<
    string,
    Promise<NativeReactFrontendModuleLoadResult>
  >();
  const selectiveFlights = new Map<
    string,
    Promise<NativeReactFrontendModuleLoadResult>
  >();
  const loadRegistration = (
    moduleSource: string
  ): Promise<NativeReactFrontendModuleLoadResult> => {
    const registration = registrations.get(moduleSource);
    if (!registration) {
      return Promise.reject(
        registryError(
          'module_not_registered',
          `modules.${moduleSource}`,
          `Frontend module is not registered: ${moduleSource}.`
        )
      );
    }
    let flight = flights.get(moduleSource);
    if (!flight) {
      flight = Promise.resolve()
        .then(() => registration.load())
        .then((loaded) => validateLoadedModule(registration, loaded))
        .catch((error) => {
          flights.delete(moduleSource);
          if (error instanceof NativeReactModuleRegistryError) throw error;
          throw registryError(
            'module_load_failed',
            `modules.${moduleSource}`,
            error instanceof Error && error.message
              ? error.message
              : `Frontend module failed to load: ${moduleSource}.`
          );
        });
      flights.set(moduleSource, flight);
    }
    return flight;
  };

  const loadInjectedRegistration = (
    injectedModule: NativeTrustedBlockInjectedModule
  ): Promise<NativeReactFrontendModuleLoadResult> => {
    const { source: moduleSource, bindings } = injectedModule;
    const registration = registrations.get(moduleSource);
    if (
      !registration?.loadExports ||
      bindings.length === 0 ||
      bindings.some((binding) => binding.kind === 'namespace')
    ) {
      return loadRegistration(moduleSource);
    }
    const existingFullFlight = flights.get(moduleSource);
    if (existingFullFlight) return existingFullFlight;

    const requestedExports = [
      ...new Set(
        bindings.flatMap((binding) =>
          binding.kind === 'namespace'
            ? []
            : [binding.kind === 'default' ? 'default' : binding.imported]
        )
      )
    ].sort();
    const selectiveKey = `${moduleSource}\0${requestedExports.join('\0')}`;
    let flight = selectiveFlights.get(selectiveKey);
    if (!flight) {
      flight = Promise.resolve()
        .then(() => registration.loadExports!(requestedExports))
        .then((loaded) =>
          validateLoadedModule(registration, loaded, requestedExports)
        )
        .catch((error) => {
          selectiveFlights.delete(selectiveKey);
          if (error instanceof NativeReactModuleRegistryError) throw error;
          throw registryError(
            'module_load_failed',
            `modules.${moduleSource}`,
            error instanceof Error && error.message
              ? error.message
              : `Frontend module failed to load: ${moduleSource}.`
          );
        });
      selectiveFlights.set(selectiveKey, flight);
    }
    return flight;
  };

  const toStyleAssets = (
    moduleSource: string,
    loaded: NativeReactFrontendModuleLoadResult
  ): NativeReactResolvedModuleAsset[] =>
    (loaded.styles ?? []).map((style) => {
      const bytes = new TextEncoder().encode(style.css);
      const sha256 = sha256Text(style.css);
      return {
        module_source: moduleSource,
        role: 'shadow_style' as const,
        media_type: style.media_type ?? 'text/css; charset=utf-8',
        sha256,
        url: `frontend-module-style:${sha256}`,
        bytes: bytes.buffer
      };
    });

  const mergeInjectedModules = (
    injectedModules: readonly NativeTrustedBlockInjectedModule[]
  ): NativeTrustedBlockInjectedModule[] => {
    const bySource = new Map<string, NativeTrustedBlockInjectedModule>();
    for (const item of injectedModules) {
      const prev = bySource.get(item.source);
      bySource.set(
        item.source,
        prev
          ? { source: item.source, bindings: [...prev.bindings, ...item.bindings] }
          : item
      );
    }
    return [...bySource.values()];
  };

  return {
    definitions,
    async load(moduleSource) {
      return (await loadRegistration(moduleSource)).module;
    },
    async resolveModuleMap(moduleSources) {
      const uniqueSources = [...new Set(moduleSources)];
      const loaded = await Promise.all(uniqueSources.map(loadRegistration));
      return Object.fromEntries(
        uniqueSources.map((source, index) => [source, loaded[index]!.module])
      );
    },
    async resolveModuleAssets(moduleSources) {
      const uniqueSources = [...new Set(moduleSources)];
      const loaded = await Promise.all(uniqueSources.map(loadRegistration));
      return uniqueSources.flatMap((moduleSource, index) =>
        toStyleAssets(moduleSource, loaded[index]!)
      );
    },
    async resolveInjectedModuleMap(injectedModules) {
      const merged = mergeInjectedModules(injectedModules);
      const loaded = await Promise.all(merged.map(loadInjectedRegistration));
      return Object.fromEntries(
        merged.map((item, index) => [item.source, loaded[index]!.module])
      );
    },
    async resolveInjectedModuleAssets(injectedModules) {
      const merged = mergeInjectedModules(injectedModules);
      const loaded = await Promise.all(merged.map(loadInjectedRegistration));
      return merged.flatMap((item, index) =>
        toStyleAssets(item.source, loaded[index]!)
      );
    }
  };
}

function validateLoadedModule(
  registration: NativeReactFrontendModuleRegistration,
  value: NativeReactFrontendModuleLoadResult,
  requestedExports?: readonly string[]
): NativeReactFrontendModuleLoadResult {
  if (!isRecord(value) || !isRecord(value.module)) {
    throw registryError(
      'module_load_failed',
      `modules.${registration.module_source}`,
      `Frontend module returned an invalid namespace: ${registration.module_source}.`
    );
  }
  const allowsWildcard = registration.exports.includes('*');
  const required = requestedExports ?? registration.exports;
  for (const exportName of required) {
    if (exportName === '*') continue;
    if (!allowsWildcard && !registration.exports.includes(exportName)) {
      throw registryError(
        'module_export_missing',
        `modules.${registration.module_source}.${exportName}`,
        `Frontend module export is unavailable: ${registration.module_source}.${exportName}.`
      );
    }
    if (!(exportName in value.module)) {
      throw registryError(
        'module_export_missing',
        `modules.${registration.module_source}.${exportName}`,
        `Frontend module export is unavailable: ${registration.module_source}.${exportName}.`
      );
    }
  }
  if (
    value.styles !== undefined &&
    (!Array.isArray(value.styles) ||
      value.styles.some(
        (style) =>
          !isRecord(style) ||
          typeof style.css !== 'string' ||
          (style.media_type !== undefined &&
            style.media_type !== 'text/css' &&
            style.media_type !== 'text/css; charset=utf-8')
      ))
  ) {
    throw registryError(
      'module_load_failed',
      `modules.${registration.module_source}.styles`,
      `Frontend module styles are invalid: ${registration.module_source}.`
    );
  }
  return value;
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
