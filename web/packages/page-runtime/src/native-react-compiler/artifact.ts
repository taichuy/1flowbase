import type { BlockProtocolError } from '@1flowbase/page-protocol';

import { NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS } from '../native-trusted-block-source-policy';
import type {
  NativeTrustedBlockImportBinding,
  NativeTrustedBlockInjectedModule
} from '../native-trusted-block/source-evaluator';
import { NATIVE_REACT_JSX_RUNTIME_IMPORT_SOURCE } from '../native-trusted-block/source-evaluator-types';
import {
  sha256Text,
  type JsonValue
} from '../js-block-runtime/compiled-artifact';
import { transformNativeReactComponentSource } from './component-transform';

export const NATIVE_REACT_COMPONENT_ARTIFACT_FORMAT =
  '1flowbase/native-react-component' as const;
export const NATIVE_REACT_COMPONENT_ARTIFACT_VERSION = 1 as const;

export interface NativeReactCompileDiagnostic extends BlockProtocolError {
  phase: 'compile';
}

export interface NativeReactComponentArtifact {
  format: typeof NATIVE_REACT_COMPONENT_ARTIFACT_FORMAT;
  version: typeof NATIVE_REACT_COMPONENT_ARTIFACT_VERSION;
  sourceSha256: string;
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
  source: unknown
): NativeReactComponentCompileResult {
  const transformed = transformNativeReactComponentSource(source);
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

  return {
    ok: true,
    artifact: {
      format: NATIVE_REACT_COMPONENT_ARTIFACT_FORMAT,
      version: NATIVE_REACT_COMPONENT_ARTIFACT_VERSION,
      sourceSha256: sha256Text(transformed.source),
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
    },
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
    !isSha256(value.sourceSha256) ||
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
  if (
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

  return {
    format: NATIVE_REACT_COMPONENT_ARTIFACT_FORMAT,
    version: NATIVE_REACT_COMPONENT_ARTIFACT_VERSION,
    sourceSha256: value.sourceSha256,
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
    if (!isRecord(item) || !isAllowedImport(item.source)) return null;
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
      !isAllowedImport(item.source) ||
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
    ? { ...value }
    : { kind: value.kind, source: value.source, local: value.local };
}

function readStringArray(value: unknown): string[] | null {
  return Array.isArray(value) && value.every(isNonEmptyString)
    ? [...value]
    : null;
}

function isAllowedImport(
  value: unknown
): value is NativeTrustedBlockInjectedModule['source'] {
  return (
    typeof value === 'string' &&
    ((NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS as readonly string[]).includes(
      value
    ) ||
      value === NATIVE_REACT_JSX_RUNTIME_IMPORT_SOURCE)
  );
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
