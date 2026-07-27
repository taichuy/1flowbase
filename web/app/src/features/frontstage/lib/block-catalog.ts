import { frontstageComponentModuleAssetPath } from '@1flowbase/api-client';
import {
  FRONTEND_BLOCK_CONTEXT_PRIMITIVES,
  FRONTEND_BLOCK_RUNTIMES,
  FRONTEND_BLOCK_UI_CAPABILITIES,
  type FrontendBlockContextPrimitive,
  type FrontendBlockRuntime,
  type FrontendBlockUiCapability
} from '@1flowbase/page-protocol';
import { createFrontendBlockCodeCapabilities } from '@1flowbase/page-protocol';
import {
  canonicalizeNativeReactCatalogDependencyLock,
  type NativeReactCatalogDependencyLock
} from '@1flowbase/page-runtime';

import type { FrontstageBlockCatalogEntry } from '../api/block-catalog';

export const FRONTSTAGE_BLOCK_RUNTIME_KINDS = FRONTEND_BLOCK_RUNTIMES;
export const FRONTSTAGE_BLOCK_CONTEXT_PRIMITIVES =
  FRONTEND_BLOCK_CONTEXT_PRIMITIVES;
export const FRONTSTAGE_BLOCK_UI_CAPABILITIES = FRONTEND_BLOCK_UI_CAPABILITIES;

export type FrontstageBlockRuntimeKind = FrontendBlockRuntime;
export type FrontstageBlockContextPrimitive = FrontendBlockContextPrimitive;
export type FrontstageBlockUiCapability = FrontendBlockUiCapability;
export type FrontstageBlockPermissionChannel = 'data' | 'action' | 'event';

export type FrontstageBlockCatalogDiagnosticSeverity = 'warning' | 'error';
export type FrontstageBlockCatalogDiagnosticCode =
  | 'unknown_runtime'
  | 'unknown_primitive'
  | 'unknown_capability'
  | 'invalid_code_module';

export interface FrontstageBlockCatalogDiagnostic {
  severity: FrontstageBlockCatalogDiagnosticSeverity;
  code: FrontstageBlockCatalogDiagnosticCode;
  providerCode: string;
  pluginId: string;
  contributionCode: string;
  field: string;
  value: string;
  message: string;
}

export interface NormalizedFrontstageBlockPermissions {
  network: string;
  storage: string;
  secrets: string;
}

export interface NormalizedFrontstageBlockContextContract {
  primitives: FrontstageBlockContextPrimitive[];
  inputSchema: Record<string, unknown>;
}

export interface NormalizedFrontstageBlockCodeModule {
  source: string;
  version: string;
  binding: 'host' | 'fetched';
  assets: Array<{
    role: 'browser_module' | 'shadow_style' | 'support';
    media_type: string;
    sha256: string;
  }>;
  exports: string[];
  type_declarations: string;
}

export interface NormalizedFrontstageBlockCatalogEntry {
  id: string;
  runtimeKind: FrontstageBlockRuntimeKind;
  installationId: string;
  providerCode: string;
  pluginId: string;
  pluginVersion: string;
  contributionCode: string;
  title: string;
  entry: string;
  permissions: NormalizedFrontstageBlockPermissions;
  contextContract: NormalizedFrontstageBlockContextContract;
  uiCapabilities: FrontstageBlockUiCapability[];
  codeModules?: NormalizedFrontstageBlockCodeModule[] | null;
  codeCapabilities?: ReturnType<typeof createFrontendBlockCodeCapabilities>;
  raw: FrontstageBlockCatalogEntry;
}

export interface FrontstageBlockCatalogNormalizationResult {
  items: NormalizedFrontstageBlockCatalogEntry[];
  diagnostics: FrontstageBlockCatalogDiagnostic[];
}

const runtimeKinds = new Set<string>(FRONTSTAGE_BLOCK_RUNTIME_KINDS);
const contextPrimitives = new Set<string>(FRONTSTAGE_BLOCK_CONTEXT_PRIMITIVES);
const uiCapabilities = new Set<string>(FRONTSTAGE_BLOCK_UI_CAPABILITIES);

export function normalizeFrontstageBlockCatalog(
  entries: FrontstageBlockCatalogEntry[]
): FrontstageBlockCatalogNormalizationResult {
  const diagnostics: FrontstageBlockCatalogDiagnostic[] = [];
  const items: NormalizedFrontstageBlockCatalogEntry[] = [];

  for (const entry of entries) {
    const diagnosticBase = getDiagnosticBase(entry);
    const runtimeKind = normalizeRuntimeKind(entry.runtime);

    if (!runtimeKind) {
      diagnostics.push({
        ...diagnosticBase,
        severity: 'error',
        code: 'unknown_runtime',
        field: 'runtime',
        value: entry.runtime,
        message: `Unsupported frontstage block runtime "${entry.runtime}"; entry was filtered.`
      });
      continue;
    }

    const primitives = filterKnownValues(
      entry.context_contract.primitives,
      contextPrimitives,
      (value) => {
        diagnostics.push({
          ...diagnosticBase,
          severity: 'warning',
          code: 'unknown_primitive',
          field: 'context_contract.primitives',
          value,
          message: `Unsupported frontstage block context primitive "${value}"; primitive was ignored.`
        });
      }
    ) as FrontstageBlockContextPrimitive[];

    const capabilities = filterKnownValues(
      entry.ui_capabilities,
      uiCapabilities,
      (value) => {
        diagnostics.push({
          ...diagnosticBase,
          severity: 'warning',
          code: 'unknown_capability',
          field: 'ui_capabilities',
          value,
          message: `Unsupported frontstage block UI capability "${value}"; capability was ignored.`
        });
      }
    ) as FrontstageBlockUiCapability[];

    const codeModules = normalizeCodeModules(entry.code_modules);
    if (codeModules === null) {
      diagnostics.push({
        ...diagnosticBase,
        severity: 'error',
        code: 'invalid_code_module',
        field: 'code_modules',
        value: entry.contribution_code,
        message:
          'Frontend block catalog code_modules must include source, version, binding, digest-locked assets, exports, and type_declarations.'
      });
    }
    const codeCapabilities = createFrontendBlockCodeCapabilities({
      code_template: entry.code_template,
      code_template_version: entry.code_template_version,
      code_template_language: entry.code_template_language,
      code_modules: (codeModules ?? []).map((codeModule) => ({
        source: codeModule.source,
        type_declarations: codeModule.type_declarations
      }))
    });
    items.push({
      id: `${entry.provider_code}:${entry.contribution_code}`,
      runtimeKind,
      installationId: entry.installation_id,
      providerCode: entry.provider_code,
      pluginId: entry.plugin_id,
      pluginVersion: entry.plugin_version,
      contributionCode: entry.contribution_code,
      title: entry.title,
      entry: entry.entry,
      permissions: {
        network: entry.permissions.network,
        storage: entry.permissions.storage,
        secrets: entry.permissions.secrets
      },
      contextContract: {
        primitives,
        inputSchema: entry.context_contract.input_schema
      },
      uiCapabilities: capabilities,
      codeModules,
      ...(codeCapabilities.template ||
      codeCapabilities.allowedImports.length > 0
        ? { codeCapabilities }
        : {}),
      raw: entry
    });
  }

  return { items, diagnostics };
}

export interface FrontstageNativeDependencyLockResolution {
  dependencyLock: NativeReactCatalogDependencyLock;
  error: string | null;
}

export function resolveFrontstageNativeDependencyLock({
  catalogEntry,
  workspaceId
}: {
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
  workspaceId: string;
}): FrontstageNativeDependencyLockResolution {
  if (!catalogEntry) return { dependencyLock: [], error: null };
  if (!catalogEntry.codeModules) {
    return {
      dependencyLock: [],
      error:
        'Frontend block catalog dependency metadata is incomplete for this block.'
    };
  }

  const dependencyLock = canonicalizeNativeReactCatalogDependencyLock(
    catalogEntry.codeModules.map((codeModule) => ({
      module_source: codeModule.source,
      module_version: codeModule.version,
      binding: codeModule.binding,
      assets: codeModule.assets.map((asset) => ({
        ...asset,
        url: frontstageComponentModuleAssetPath(workspaceId, asset.sha256)
      })),
      exports: codeModule.exports
    }))
  );
  return dependencyLock
    ? { dependencyLock, error: null }
    : {
        dependencyLock: [],
        error:
          'Frontend block catalog dependency metadata is invalid for this block.'
      };
}

export function isFrontstageBlockNativeRuntime(
  entry: NormalizedFrontstageBlockCatalogEntry | FrontstageBlockRuntimeKind
): boolean {
  return getRuntimeKind(entry) === 'native_react';
}

export function supportsFrontstageBlockCapability(
  entry: NormalizedFrontstageBlockCatalogEntry,
  capability: FrontstageBlockUiCapability
): boolean {
  return entry.uiCapabilities.includes(capability);
}

export function supportsFrontstageBlockPrimitive(
  entry: NormalizedFrontstageBlockCatalogEntry,
  primitive: FrontstageBlockContextPrimitive
): boolean {
  return entry.contextContract.primitives.includes(primitive);
}

export function hasFrontstageBlockPermission(
  entry: NormalizedFrontstageBlockCatalogEntry,
  channel: FrontstageBlockPermissionChannel
): boolean {
  switch (channel) {
    case 'data':
      return (
        supportsFrontstageBlockPrimitive(entry, 'data_record') ||
        supportsFrontstageBlockCapability(entry, 'data_binding')
      );
    case 'action':
      return supportsFrontstageBlockPrimitive(entry, 'button');
    case 'event':
      return false;
    default:
      return false;
  }
}

export function hasFrontstageBlockDataPermission(
  entry: NormalizedFrontstageBlockCatalogEntry
): boolean {
  return hasFrontstageBlockPermission(entry, 'data');
}

export function hasFrontstageBlockActionPermission(
  entry: NormalizedFrontstageBlockCatalogEntry
): boolean {
  return hasFrontstageBlockPermission(entry, 'action');
}

export function hasFrontstageBlockEventPermission(
  entry: NormalizedFrontstageBlockCatalogEntry
): boolean {
  return hasFrontstageBlockPermission(entry, 'event');
}

export function filterFrontstageBlockCatalogByRuntime(
  entries: NormalizedFrontstageBlockCatalogEntry[],
  runtimeKind: FrontstageBlockRuntimeKind
): NormalizedFrontstageBlockCatalogEntry[] {
  return entries.filter((entry) => entry.runtimeKind === runtimeKind);
}

export function filterFrontstageBlockCatalogByCapability(
  entries: NormalizedFrontstageBlockCatalogEntry[],
  capability: FrontstageBlockUiCapability
): NormalizedFrontstageBlockCatalogEntry[] {
  return entries.filter((entry) =>
    supportsFrontstageBlockCapability(entry, capability)
  );
}

function normalizeRuntimeKind(
  value: string
): FrontstageBlockRuntimeKind | undefined {
  if (!runtimeKinds.has(value)) {
    return undefined;
  }
  return value as FrontstageBlockRuntimeKind;
}

function filterKnownValues(
  values: string[],
  allowedValues: Set<string>,
  onUnknown: (value: string) => void
): string[] {
  const filtered: string[] = [];
  const seen = new Set<string>();

  for (const value of values) {
    if (!allowedValues.has(value)) {
      onUnknown(value);
      continue;
    }
    if (!seen.has(value)) {
      filtered.push(value);
      seen.add(value);
    }
  }

  return filtered;
}

function getRuntimeKind(
  entry: NormalizedFrontstageBlockCatalogEntry | FrontstageBlockRuntimeKind
): FrontstageBlockRuntimeKind {
  return typeof entry === 'string' ? entry : entry.runtimeKind;
}

function getDiagnosticBase(entry: FrontstageBlockCatalogEntry) {
  return {
    providerCode: entry.provider_code,
    pluginId: entry.plugin_id,
    contributionCode: entry.contribution_code
  };
}

function normalizeCodeModules(
  value: unknown
): NormalizedFrontstageBlockCodeModule[] | null {
  if (!Array.isArray(value)) return null;
  const modules: NormalizedFrontstageBlockCodeModule[] = [];
  for (const item of value) {
    if (
      !isRecord(item) ||
      !isNonEmptyString(item.source) ||
      !isNonEmptyString(item.version) ||
      (item.binding !== 'host' && item.binding !== 'fetched') ||
      !Array.isArray(item.assets) ||
      !Array.isArray(item.exports) ||
      item.exports.length === 0 ||
      !item.exports.every(isNonEmptyString) ||
      new Set(item.exports).size !== item.exports.length ||
      typeof item.type_declarations !== 'string'
    ) {
      return null;
    }
    const assets = normalizeModuleAssets(item.assets);
    if (!assets) return null;
    modules.push({
      source: item.source,
      version: item.version,
      binding: item.binding,
      assets,
      exports: [...item.exports],
      type_declarations: item.type_declarations
    });
  }
  return modules;
}

function normalizeModuleAssets(
  value: unknown[]
): NormalizedFrontstageBlockCodeModule['assets'] | null {
  const assets: NormalizedFrontstageBlockCodeModule['assets'] = [];
  for (const item of value) {
    if (
      !isRecord(item) ||
      (item.role !== 'browser_module' &&
        item.role !== 'shadow_style' &&
        item.role !== 'support') ||
      !isNonEmptyString(item.media_type) ||
      !isSha256(item.sha256)
    ) {
      return null;
    }
    assets.push({
      role: item.role,
      media_type: item.media_type,
      sha256: item.sha256
    });
  }
  return assets;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}

function isSha256(value: unknown): value is string {
  return typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
}
