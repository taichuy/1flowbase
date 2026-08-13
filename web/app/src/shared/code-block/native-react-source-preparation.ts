import {
  evaluateNativeReactComponentArtifactWithRegistry,
  NativeReactModuleRegistryError,
  type NativeReactCatalogDependencyLock,
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
import {
  compileNativeReactExecutableStyle,
  createNativeReactExecutableStyleAsset
} from './native-react-executable-style';

export type NativeReactSourcePreparationDiagnostic =
  | NativeReactCompileDiagnostic
  | NativeReactRuntimeDiagnostic;

export type NativeReactSourcePreparationResult =
  | {
      ok: true;
      artifact: NativeReactComponentArtifact;
      component: NativeTrustedBlockComponent;
      moduleAssets: NativeReactResolvedModuleAsset[];
      executableStyle: Awaited<
        ReturnType<typeof compileNativeReactExecutableStyle>
      >;
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
  runtimeFingerprint,
  executableStyle,
  compiler = compileNativeReactComponentInBrowser,
  workerFactory,
  registryFactory,
  evaluationBindings
}: {
  frozenSource: string;
  requestId: string;
  dependencyLock: NativeReactCatalogDependencyLock;
  runtimeFingerprint?: string;
  executableStyle?: Awaited<
    ReturnType<typeof compileNativeReactExecutableStyle>
  >;
  compiler?: typeof compileNativeReactComponentInBrowser;
  workerFactory?: NativeReactBrowserCompilerWorkerFactory;
  registryFactory: NativeReactModuleRegistryFactory;
  evaluationBindings?: NativeReactArtifactEvaluationBindings;
}): Promise<NativeReactSourcePreparationResult> {
  let preparedExecutableStyle: Awaited<
    ReturnType<typeof compileNativeReactExecutableStyle>
  >;
  try {
    preparedExecutableStyle =
      executableStyle ??
      (await compileNativeReactExecutableStyle(frozenSource, dependencyLock));
  } catch (error) {
    return {
      ok: false,
      diagnostics: [
        {
          phase: 'compile',
          code: 'transform_failed',
          path: 'tailwind',
          message:
            error instanceof Error
              ? error.message
              : 'Tailwind compilation failed.'
        }
      ]
    };
  }
  const compiled = await compiler({
    source: frozenSource,
    requestId,
    dependencyLock,
    ...(runtimeFingerprint ? { runtimeFingerprint } : {}),
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
      artifact: evaluated.artifact,
      component: evaluated.component,
      moduleAssets: [
        ...moduleAssets,
        createNativeReactExecutableStyleAsset(
          preparedExecutableStyle.generated_css,
          preparedExecutableStyle.generated_css_sha256
        )
      ],
      executableStyle: preparedExecutableStyle
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
