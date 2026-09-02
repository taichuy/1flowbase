import type { BlockProtocolError } from '@1flowbase/page-protocol';

import type {
  NativeTrustedBlockImportBinding,
  NativeTrustedBlockInjectedModule
} from '../native-trusted-block/source-evaluator';
import { sha256Text } from '../sha256';
import { transformNativeReactComponentSource } from './component-transform';
import {
  canonicalizeNativeReactModuleDefinitions,
  type NativeReactModuleDefinition
} from './module-registry/contracts';
import { diagnoseLegacyBlockModuleSource } from './source-contract';

export { sha256Bytes, sha256Text } from '../sha256';
export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

export const NATIVE_REACT_COMPONENT_ARTIFACT_FORMAT =
  '1flowbase/native-react-component' as const;
export const NATIVE_REACT_COMPONENT_ARTIFACT_VERSION = 3 as const;
export const NATIVE_REACT_COMPILER_ABI =
  '1flowbase/native-react-compiler@4' as const;
export const NATIVE_REACT_RUNTIME_ABI =
  '1flowbase/native-react-runtime@4' as const;

export interface NativeReactComponentArtifactIdentity {
  source_sha256: string;
  compiler_abi: typeof NATIVE_REACT_COMPILER_ABI;
  runtime_abi: typeof NATIVE_REACT_RUNTIME_ABI;
}

export interface NativeReactCompileDiagnostic extends BlockProtocolError {
  phase: 'compile';
}

export interface NativeReactComponentArtifact {
  format: typeof NATIVE_REACT_COMPONENT_ARTIFACT_FORMAT;
  version: typeof NATIVE_REACT_COMPONENT_ARTIFACT_VERSION;
  identity: NativeReactComponentArtifactIdentity;
  integritySha256: string;
  program: {
    injectedModules: NativeTrustedBlockInjectedModule[];
    importBindings: NativeTrustedBlockImportBinding[];
    executableBody: string;
    executablePreambleLines: number;
    moduleMapIdentifier: string;
    runtimeCapabilityGuardBindingIdentifiers: string[];
    defaultExportIdentifier: string;
  };
  sourceMap: JsonValue;
}

export type NativeReactComponentCompileResult =
  | { ok: true; artifact: NativeReactComponentArtifact; diagnostics: [] }
  | { ok: false; diagnostics: NativeReactCompileDiagnostic[] };

export function compileNativeReactComponent(
  source: unknown,
  moduleDefinitionsValue: unknown = []
): NativeReactComponentCompileResult {
  const legacyDiagnostic = diagnoseLegacyBlockModuleSource(source);
  if (legacyDiagnostic) {
    return { ok: false, diagnostics: [legacyDiagnostic] };
  }
  const moduleDefinitions =
    canonicalizeNativeReactModuleDefinitions(moduleDefinitionsValue);
  if (!moduleDefinitions) {
    return compileFailure(
      'moduleRegistry',
      'Native React frontend module definitions are invalid.'
    );
  }
  const transformed = transformNativeReactComponentSource(
    source,
    new Set(moduleDefinitions.map((entry) => entry.module_source))
  );
  if (!transformed.ok) {
    return {
      ok: false,
      diagnostics: transformed.errors.map(toCompileDiagnostic)
    };
  }

  const sourceMap = canonicalJsonValue(transformed.sourceMap);
  if (sourceMap === undefined) {
    return {
      ok: false,
      diagnostics: [
        {
          phase: 'compile',
          code: 'transform_failed',
          path: 'source.map',
          message: 'Native React component source map is not serializable.'
        }
      ]
    };
  }

  const moduleDiagnostic = validateImportedModuleDefinitions(
    transformed.injectedModules,
    moduleDefinitions
  );
  if (moduleDiagnostic) return moduleDiagnostic;
  const payload: Omit<NativeReactComponentArtifact, 'integritySha256'> = {
    format: NATIVE_REACT_COMPONENT_ARTIFACT_FORMAT,
    version: NATIVE_REACT_COMPONENT_ARTIFACT_VERSION,
    identity: createNativeReactComponentArtifactIdentity({
      sourceSha256: sha256Text(transformed.source)
    }),
    program: {
      injectedModules: transformed.injectedModules.map(cloneInjectedModule),
      importBindings: transformed.importBindings.map(cloneImportBinding),
      executableBody: transformed.executableBody,
      executablePreambleLines: transformed.executablePreambleLines,
      moduleMapIdentifier: transformed.moduleMapIdentifier,
      runtimeCapabilityGuardBindingIdentifiers: [
        ...transformed.runtimeCapabilityGuardBindingIdentifiers
      ],
      defaultExportIdentifier: transformed.defaultExportIdentifier
    },
    sourceMap
  };
  return {
    ok: true,
    artifact: { ...payload, integritySha256: artifactIntegrity(payload) },
    diagnostics: []
  };
}

export function canonicalizeNativeReactComponentArtifact(
  value: unknown
): NativeReactComponentArtifact | null {
  if (
    !isRecord(value) ||
    value.format !== NATIVE_REACT_COMPONENT_ARTIFACT_FORMAT ||
    value.version !== NATIVE_REACT_COMPONENT_ARTIFACT_VERSION ||
    !isRecord(value.identity) ||
    !isSha256(value.integritySha256) ||
    !isRecord(value.program)
  ) {
    return null;
  }

  const program = value.program;
  const injectedModules = readInjectedModules(program.injectedModules);
  const importBindings = readImportBindings(program.importBindings);
  const guardIdentifiers = readStringArray(
    program.runtimeCapabilityGuardBindingIdentifiers
  );
  const sourceMap = canonicalJsonValue(value.sourceMap);
  const identity = readArtifactIdentity(value.identity);
  if (
    !identity ||
    !injectedModules ||
    !importBindings ||
    !guardIdentifiers ||
    typeof program.executableBody !== 'string' ||
    !Number.isSafeInteger(program.executablePreambleLines) ||
    (program.executablePreambleLines as number) < 0 ||
    !isNonEmptyString(program.moduleMapIdentifier) ||
    !isNonEmptyString(program.defaultExportIdentifier) ||
    sourceMap === undefined
  ) {
    return null;
  }

  const payload: Omit<NativeReactComponentArtifact, 'integritySha256'> = {
    format: NATIVE_REACT_COMPONENT_ARTIFACT_FORMAT,
    version: NATIVE_REACT_COMPONENT_ARTIFACT_VERSION,
    identity,
    program: {
      injectedModules,
      importBindings,
      executableBody: program.executableBody,
      executablePreambleLines: program.executablePreambleLines as number,
      moduleMapIdentifier: program.moduleMapIdentifier,
      runtimeCapabilityGuardBindingIdentifiers: guardIdentifiers,
      defaultExportIdentifier: program.defaultExportIdentifier
    },
    sourceMap
  };
  return artifactIntegrity(payload) === value.integritySha256
    ? { ...payload, integritySha256: value.integritySha256 }
    : null;
}

export function createNativeReactComponentArtifactIdentity({
  sourceSha256
}: {
  sourceSha256: string;
}): NativeReactComponentArtifactIdentity {
  return {
    source_sha256: sourceSha256,
    compiler_abi: NATIVE_REACT_COMPILER_ABI,
    runtime_abi: NATIVE_REACT_RUNTIME_ABI
  };
}

export function nativeReactComponentArtifactMatchesIdentity(
  artifact: NativeReactComponentArtifact,
  identity: NativeReactComponentArtifactIdentity
): boolean {
  return (
    artifact.identity.source_sha256 === identity.source_sha256 &&
    artifact.identity.compiler_abi === identity.compiler_abi &&
    artifact.identity.runtime_abi === identity.runtime_abi
  );
}

function readArtifactIdentity(
  value: Record<string, unknown>
): NativeReactComponentArtifactIdentity | null {
  if (
    !isSha256(value.source_sha256) ||
    value.compiler_abi !== NATIVE_REACT_COMPILER_ABI ||
    value.runtime_abi !== NATIVE_REACT_RUNTIME_ABI
  ) {
    return null;
  }
  return {
    source_sha256: value.source_sha256,
    compiler_abi: NATIVE_REACT_COMPILER_ABI,
    runtime_abi: NATIVE_REACT_RUNTIME_ABI
  };
}

function artifactIntegrity(value: object): string {
  return sha256Text(JSON.stringify(value));
}

function toCompileDiagnostic(
  error: BlockProtocolError
): NativeReactCompileDiagnostic {
  return { phase: 'compile', ...error };
}

function readInjectedModules(
  value: unknown
): NativeTrustedBlockInjectedModule[] | null {
  if (!Array.isArray(value)) return null;
  const modules = value.map((item) => {
    if (!isRecord(item) || !isNonEmptyString(item.source)) return null;
    const bindings = readImportBindings(item.bindings);
    return bindings &&
      bindings.every((binding) => binding.source === item.source)
      ? { source: item.source, bindings }
      : null;
  });
  return modules.some((item) => item === null)
    ? null
    : (modules as NativeTrustedBlockInjectedModule[]);
}

function readImportBindings(
  value: unknown
): NativeTrustedBlockImportBinding[] | null {
  if (!Array.isArray(value)) return null;
  const bindings = value.map((item) => {
    if (
      !isRecord(item) ||
      !isNonEmptyString(item.source) ||
      !isNonEmptyString(item.local)
    ) {
      return null;
    }
    if (item.kind === 'named' && isNonEmptyString(item.imported)) {
      return {
        kind: 'named' as const,
        source: item.source,
        imported: item.imported,
        local: item.local
      };
    }
    if (item.kind === 'default' || item.kind === 'namespace') {
      return { kind: item.kind, source: item.source, local: item.local };
    }
    return null;
  });
  return bindings.some((item) => item === null)
    ? null
    : (bindings as NativeTrustedBlockImportBinding[]);
}

function cloneInjectedModule(
  value: NativeTrustedBlockInjectedModule
): NativeTrustedBlockInjectedModule {
  return {
    source: value.source,
    bindings: value.bindings.map(cloneImportBinding)
  };
}

function cloneImportBinding(
  value: NativeTrustedBlockImportBinding
): NativeTrustedBlockImportBinding {
  return value.kind === 'named'
    ? {
        kind: 'named',
        source: value.source,
        imported: value.imported,
        local: value.local
      }
    : { kind: value.kind, source: value.source, local: value.local };
}

function readStringArray(value: unknown): string[] | null {
  return Array.isArray(value) && value.every(isNonEmptyString)
    ? [...value]
    : null;
}

function validateImportedModuleDefinitions(
  injectedModules: NativeTrustedBlockInjectedModule[],
  moduleDefinitions: NativeReactModuleDefinition[]
): { ok: false; diagnostics: NativeReactCompileDiagnostic[] } | null {
  const registered = new Map(
    moduleDefinitions.map((entry) => [entry.module_source, entry])
  );
  for (const injectedModule of injectedModules) {
    const registration = registered.get(injectedModule.source);
    if (!registration) {
      return compileFailure(
        `moduleRegistry.${injectedModule.source}`,
        `Frontend module is not registered: ${injectedModule.source}.`
      );
    }
    for (const binding of injectedModule.bindings) {
      if (binding.kind === 'namespace') continue;
      const exportName =
        binding.kind === 'default' ? 'default' : binding.imported;
      if (
        !registration.exports.includes('*') &&
        !registration.exports.includes(exportName)
      ) {
        return compileFailure(
          `moduleRegistry.${registration.module_source}.exports.${exportName}`,
          `Frontend module export is not registered: ${registration.module_source}.${exportName}.`
        );
      }
    }
  }
  return null;
}

function compileFailure(
  path: string,
  message: string
): { ok: false; diagnostics: NativeReactCompileDiagnostic[] } {
  return {
    ok: false,
    diagnostics: [{ phase: 'compile', code: 'transform_failed', path, message }]
  };
}

function canonicalJsonValue(
  value: unknown,
  seen = new WeakSet<object>()
): JsonValue | undefined {
  if (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'boolean'
  ) {
    return value;
  }
  if (typeof value === 'number')
    return Number.isFinite(value) ? value : undefined;
  if (typeof value !== 'object' || seen.has(value)) return undefined;
  seen.add(value);
  if (Array.isArray(value)) {
    const items = value.map((item) => canonicalJsonValue(item, seen));
    return items.some((item) => item === undefined)
      ? undefined
      : (items as JsonValue[]);
  }
  const output: Record<string, JsonValue> = {};
  for (const [key, item] of Object.entries(value)) {
    const canonical = canonicalJsonValue(item, seen);
    if (canonical === undefined) return undefined;
    output[key] = canonical;
  }
  return output;
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
