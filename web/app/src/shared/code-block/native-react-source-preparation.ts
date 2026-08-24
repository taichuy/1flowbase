import {
  evaluateNativeReactComponentArtifactWithRegistry,
  NativeReactModuleRegistryError,
  type NativeReactComponentArtifact,
  type NativeReactArtifactEvaluationBindings,
  type NativeReactCompileDiagnostic,
  type NativeReactModuleRegistry,
  type NativeReactResolvedModuleAsset,
  type NativeReactRuntimeDiagnostic,
  type NativeTrustedBlockComponent
} from '@1flowbase/page-runtime';

import {
  compileNativeReactComponentInBrowser,
  type NativeReactBrowserCompilerWorkerFactory
} from './native-react-compiler-browser';

export type NativeReactSourcePreparationDiagnostic =
  | NativeReactCompileDiagnostic
  | NativeReactRuntimeDiagnostic;

export type NativeReactSourcePreparationResult =
  | {
      ok: true;
      artifact: NativeReactComponentArtifact;
      component: NativeTrustedBlockComponent;
      moduleAssets: NativeReactResolvedModuleAsset[];
    }
  | {
      ok: false;
      diagnostics: NativeReactSourcePreparationDiagnostic[];
    };

export type NativeReactModuleRegistryFactory = () => NativeReactModuleRegistry;

export async function prepareNativeReactSource({
  frozenSource,
  requestId,
  compiler = compileNativeReactComponentInBrowser,
  workerFactory,
  registryFactory,
  evaluationBindings
}: {
  frozenSource: string;
  requestId: string;
  compiler?: typeof compileNativeReactComponentInBrowser;
  workerFactory?: NativeReactBrowserCompilerWorkerFactory;
  registryFactory: NativeReactModuleRegistryFactory;
  evaluationBindings?: NativeReactArtifactEvaluationBindings;
}): Promise<NativeReactSourcePreparationResult> {
  let registry: NativeReactModuleRegistry;
  try {
    registry = registryFactory();
  } catch (error) {
    return registryFailure(error);
  }
  const compiled = await compiler({
    source: frozenSource,
    requestId,
    moduleDefinitions: registry.definitions,
    ...(workerFactory ? { workerFactory } : {})
  });
  if (!compiled.ok) return compiled;

  const evaluated = await evaluateNativeReactComponentArtifactWithRegistry(
    compiled.artifact,
    registry,
    evaluationBindings
  );
  if (!evaluated.ok) return evaluated;

  try {
    const moduleAssets = await registry.resolveModuleAssets(
      evaluated.artifact.program.injectedModules.map(({ source }) => source)
    );
    return {
      ok: true,
      artifact: evaluated.artifact,
      component: evaluated.component,
      moduleAssets
    };
  } catch (error) {
    return registryFailure(error);
  }
}

function registryFailure(
  error: unknown
): Extract<NativeReactSourcePreparationResult, { ok: false }> {
  const diagnostic: NativeReactRuntimeDiagnostic = {
    phase: 'runtime',
    code: 'runtime_error',
    path:
      error instanceof NativeReactModuleRegistryError
        ? error.path
        : 'moduleRegistry',
    message:
      error instanceof NativeReactModuleRegistryError
        ? error.message
        : 'Native React module registry failed.'
  };
  return { ok: false, diagnostics: [diagnostic] };
}
