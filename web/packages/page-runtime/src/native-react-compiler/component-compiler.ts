import type { BlockProtocolError } from '@1flowbase/page-protocol';
import type {
  NativeTrustedBlockInjectedModule,
  NativeTrustedBlockImportBinding
} from '../native-trusted-block/source-evaluator-types';
import { transformNativeReactComponentSource } from './component-transform';
import {
  canonicalizeNativeReactModuleDefinitions,
  type NativeReactModuleDefinition
} from './module-registry/contracts';
import { diagnoseLegacyBlockModuleSource } from './source-contract';
import {
  NATIVE_REACT_COMPONENT_ARTIFACT_FORMAT,
  NATIVE_REACT_COMPONENT_ARTIFACT_VERSION,
  createNativeReactComponentArtifactIdentity,
  sha256Text,
  canonicalJsonValue,
  artifactIntegrity,
  type NativeReactComponentCompileResult,
  type NativeReactComponentArtifact,
  type NativeReactCompileDiagnostic
} from './artifact-contract';

export function compileNativeReactComponent(
  source: unknown,
  moduleDefinitionsValue: unknown = []
): NativeReactComponentCompileResult {
  const legacyDiagnostic = diagnoseLegacyBlockModuleSource(source);
  if (legacyDiagnostic) {
    return { ok: false, diagnostics: [legacyDiagnostic] };
  }
  const moduleDefinitions = canonicalizeNativeReactModuleDefinitions(
    moduleDefinitionsValue
  );
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

function toCompileDiagnostic(
  error: BlockProtocolError
): NativeReactCompileDiagnostic {
  return { phase: 'compile', ...error };
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
