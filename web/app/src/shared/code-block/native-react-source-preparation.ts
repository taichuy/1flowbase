import {
  evaluateNativeReactComponentArtifactWithRegistry,
  NativeReactModuleRegistryError,
  type NativeReactCatalogDependencyLock,
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
      component: NativeTrustedBlockComponent;
      moduleAssets: NativeReactResolvedModuleAsset[];
    }
  | {
      ok: false;
      diagnostics: NativeReactSourcePreparationDiagnostic[];
    };

export type NativeReactModuleRegistryFactory = (
  dependencyLock: NativeReactCatalogDependencyLock
) => NativeReactModuleRegistry;

export async function prepareNativeReactSource({
  frozenSource,
  requestId,
  dependencyLock,
  compiler = compileNativeReactComponentInBrowser,
  workerFactory,
  registryFactory,
  evaluationBindings
}: {
  frozenSource: string;
  requestId: string;
  dependencyLock: NativeReactCatalogDependencyLock;
  compiler?: typeof compileNativeReactComponentInBrowser;
  workerFactory?: NativeReactBrowserCompilerWorkerFactory;
  registryFactory: NativeReactModuleRegistryFactory;
  evaluationBindings?: NativeReactArtifactEvaluationBindings;
}): Promise<NativeReactSourcePreparationResult> {
  const compiled = await compiler({
    source: frozenSource,
    requestId,
    dependencyLock,
    ...(workerFactory ? { workerFactory } : {})
  });
  if (!compiled.ok) return compiled;

  let registry: NativeReactModuleRegistry;
  try {
    registry = registryFactory(compiled.artifact.dependencyLock);
  } catch (error) {
    return registryFailure(error);
  }

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
