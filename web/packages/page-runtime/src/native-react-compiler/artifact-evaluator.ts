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
import {
  NativeReactModuleRegistryError,
  type NativeReactModuleRegistry
} from './module-registry/loader';

export interface NativeReactRuntimeDiagnostic extends BlockProtocolError {
  phase: 'runtime';
}

export interface NativeReactRuntimeConsole {
  debug(...args: unknown[]): void;
  error(...args: unknown[]): void;
  info(...args: unknown[]): void;
  log(...args: unknown[]): void;
  warn(...args: unknown[]): void;
}

export interface NativeReactArtifactEvaluationBindings {
  console: NativeReactRuntimeConsole;
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
  modules: NativeTrustedBlockInjectedModuleMap,
  bindings?: NativeReactArtifactEvaluationBindings
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
    const evaluator = createArtifactEvaluator(artifact, bindings);
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

export async function evaluateNativeReactComponentArtifactWithRegistry(
  value: unknown,
  registry: NativeReactModuleRegistry,
  bindings?: NativeReactArtifactEvaluationBindings
): Promise<NativeReactArtifactEvaluationResult> {
  const artifact = canonicalizeNativeReactComponentArtifact(value);
  if (!artifact) {
    return runtimeFailure(
      'artifact',
      'Native React component artifact is invalid.'
    );
  }
  try {
    const modules = await registry.resolveModuleMap(
      artifact.program.injectedModules.map(({ source }) => source)
    );
    return evaluateNativeReactComponentArtifact(artifact, modules, bindings);
  } catch (error) {
    return error instanceof NativeReactModuleRegistryError
      ? runtimeFailure(error.path, error.message)
      : runtimeFailure(
          'moduleRegistry',
          'Native React module registry failed.'
        );
  }
}

function createArtifactEvaluator(
  artifact: NativeReactComponentArtifact,
  bindings?: NativeReactArtifactEvaluationBindings
): (
  modules: NativeTrustedBlockInjectedModuleMap,
  ...guards: unknown[]
) => unknown {
  const parameterNames = [
    artifact.program.moduleMapIdentifier,
    ...artifact.program.runtimeCapabilityGuardBindingIdentifiers
  ];
  if (!bindings) {
    return new Function(
      ...parameterNames,
      `"use strict";\n${artifact.program.executableBody}`
    ) as (
      modules: NativeTrustedBlockInjectedModuleMap,
      ...guards: unknown[]
    ) => unknown;
  }

  const evaluatorFactory = new Function(
    '__1flowbaseRuntimeConsole',
    `"use strict";
const console = __1flowbaseRuntimeConsole;
return function (${parameterNames.join(', ')}) {
${artifact.program.executableBody}
};`
  ) as (
    runtimeConsole: NativeReactRuntimeConsole
  ) => (
    modules: NativeTrustedBlockInjectedModuleMap,
    ...guards: unknown[]
  ) => unknown;
  return evaluatorFactory(bindings.console);
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
