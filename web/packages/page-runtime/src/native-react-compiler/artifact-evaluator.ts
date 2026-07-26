import type { BlockProtocolError } from '@1flowbase/page-protocol';

import {
  RUNTIME_CAPABILITY_GUARD_BINDING_NAMES,
  createNativeTrustedBlockRuntimeCapabilityGuardBindings,
  getNativeTrustedBlockRuntimeCapabilityGuardValues,
  isNativeTrustedBlockRuntimeCapabilityGuardError
} from '../native-trusted-block/runtime-capability-guard';
import type {
  NativeTrustedBlockComponent,
  NativeTrustedBlockInjectedModuleMap
} from '../native-trusted-block/source-evaluator-types';
import {
  canonicalizeNativeReactComponentArtifact,
  type NativeReactComponentArtifact
} from './artifact';

export interface NativeReactRuntimeDiagnostic extends BlockProtocolError {
  phase: 'runtime';
}

export type NativeReactArtifactEvaluationResult =
  | {
      ok: true;
      artifact: NativeReactComponentArtifact;
      component: NativeTrustedBlockComponent;
      diagnostics: [];
    }
  | { ok: false; diagnostics: NativeReactRuntimeDiagnostic[] };

export function evaluateNativeReactComponentArtifact(
  value: unknown,
  modules: NativeTrustedBlockInjectedModuleMap
): NativeReactArtifactEvaluationResult {
  const artifact = canonicalizeNativeReactComponentArtifact(value);
  if (!artifact || !hasCanonicalGuardBindings(artifact)) {
    return runtimeFailure(
      'artifact',
      'Native React component artifact is invalid.'
    );
  }

  const moduleDiagnostic = validateInjectedModules(artifact, modules);
  if (moduleDiagnostic) return { ok: false, diagnostics: [moduleDiagnostic] };

  try {
    const guardBindings =
      createNativeTrustedBlockRuntimeCapabilityGuardBindings();
    const evaluator = new Function(
      artifact.program.moduleMapIdentifier,
      ...artifact.program.runtimeCapabilityGuardBindingIdentifiers,
      `"use strict";\n${artifact.program.executableBody}`
    ) as (
      modules: NativeTrustedBlockInjectedModuleMap,
      ...guards: unknown[]
    ) => unknown;
    const component = evaluator(
      modules,
      ...getNativeTrustedBlockRuntimeCapabilityGuardValues(guardBindings)
    );
    if (typeof component !== 'function') {
      return runtimeFailure(
        'source.defaultExport',
        'Native React default export must be a component function.'
      );
    }
    return {
      ok: true,
      artifact,
      component: component as NativeTrustedBlockComponent,
      diagnostics: []
    };
  } catch (error) {
    if (isNativeTrustedBlockRuntimeCapabilityGuardError(error)) {
      return runtimeFailure(error.path, error.message);
    }
    return runtimeFailure(
      'runtime.evaluate',
      error instanceof Error && error.message
        ? error.message
        : 'Native React artifact evaluation failed.'
    );
  }
}

function hasCanonicalGuardBindings(
  artifact: NativeReactComponentArtifact
): boolean {
  return (
    artifact.program.runtimeCapabilityGuardBindingIdentifiers.length ===
      RUNTIME_CAPABILITY_GUARD_BINDING_NAMES.length &&
    artifact.program.runtimeCapabilityGuardBindingIdentifiers.every(
      (name, index) => name === RUNTIME_CAPABILITY_GUARD_BINDING_NAMES[index]
    )
  );
}

function validateInjectedModules(
  artifact: NativeReactComponentArtifact,
  modules: NativeTrustedBlockInjectedModuleMap
): NativeReactRuntimeDiagnostic | null {
  for (const injectedModule of artifact.program.injectedModules) {
    const moduleValue = modules[injectedModule.source];
    if (!moduleValue) {
      return runtimeDiagnostic(
        `modules.${injectedModule.source}`,
        `Injected module is missing: ${injectedModule.source}.`
      );
    }
    for (const binding of injectedModule.bindings) {
      if (binding.kind === 'namespace') continue;
      const exportedName =
        binding.kind === 'default' ? 'default' : binding.imported;
      if (!(exportedName in moduleValue)) {
        return runtimeDiagnostic(
          `modules.${injectedModule.source}.${exportedName}`,
          `Injected module binding is missing: ${injectedModule.source}.${exportedName}.`
        );
      }
    }
  }
  return null;
}

function runtimeFailure(
  path: string,
  message: string
): NativeReactArtifactEvaluationResult {
  return { ok: false, diagnostics: [runtimeDiagnostic(path, message)] };
}

function runtimeDiagnostic(
  path: string,
  message: string
): NativeReactRuntimeDiagnostic {
  return {
    phase: 'runtime',
    code: 'runtime_error',
    path,
    message
  };
}
