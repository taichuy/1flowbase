import {
  evaluateNativeReactComponentArtifactWithRegistry,
  diagnoseLegacyBlockModuleSource,
  NativeReactSourceContractError,
  nativeReactCatalogDependencyLockIdentity,
  type NativeReactCatalogDependencyLock,
  type NativeReactModuleRegistry
} from '@1flowbase/page-runtime';
import {
  getFrontstageBlockCode,
  type ConsoleFrontstageBlockCode
} from '@1flowbase/api-client';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { getFrontstageApiBaseUrl } from '../api/page-tree';
import {
  compileNativeReactComponentInBrowser,
  getNativeReactRuntimeFingerprint,
  type NativeReactBrowserCompileResult
} from '../../../shared/code-block/native-react-compiler-browser';
import { readLockedNativeReactExecutableStyle } from '../../../shared/code-block/native-react-executable-style';
import { createFrontstageNativeReactModuleRegistry } from '../lib/native-trusted-block-runtime-factory';
import {
  createFrontstageNativeReactArtifactCacheIdentity,
  frontstageNativeReactArtifactCache,
  type FrontstageNativeReactArtifactCache
} from '../lib/runtime-cache';
import {
  FrontstageNativePreparationScheduler,
  FrontstagePageNativeModuleRegistryCache,
  prepareFrontstageNativeContribution,
  type FrontstageNativePreparationSnapshot,
  type FrontstageNativePreparationTask
} from '../lib/page-canvas/native-runtime-preparation';
import type { FrontstagePageCanvasBlockCodeReadPlan } from '../lib/page-canvas/runtime-source';
import type { FrontstageRuntimeDemandByBlockId } from '../lib/page-canvas/runtime-demand';
import { recordFrontstageRuntimeObservation } from '../lib/page-canvas/runtime-observation';
import {
  describeExternalNpmImportFailure,
  type ExternalNpmPackState
} from '../api/external-npm';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';

type NativePreparationSource = ConsoleFrontstageBlockCode;

export interface UseFrontstagePageCanvasNativePreparationsInput {
  actorId: string | null | undefined;
  actorWorkspaceId: string | null | undefined;
  readPlan: FrontstagePageCanvasBlockCodeReadPlan | null | undefined;
  catalogEntries?: readonly NormalizedFrontstageBlockCatalogEntry[] | null;
  externalNpm: ExternalNpmPackState;
  demandsByBlockId?: FrontstageRuntimeDemandByBlockId;
  maxConcurrent?: number;
  artifactCache?: Pick<FrontstageNativeReactArtifactCache, 'get' | 'put'>;
  runtimeFingerprint?: string;
  fetchSource?: (
    request: FrontstagePageCanvasBlockCodeReadPlan['requests'][number],
    signal: AbortSignal
  ) => Promise<NativePreparationSource>;
  compile?: (input: {
    source: string;
    requestId: string;
    dependencyLock: NativeReactCatalogDependencyLock;
    runtimeFingerprint: string;
  }) => Promise<NativeReactBrowserCompileResult>;
  moduleRegistryFactory?: (
    dependencyLock: NativeReactCatalogDependencyLock
  ) => NativeReactModuleRegistry;
}

export interface UseFrontstagePageCanvasNativePreparationsResult {
  preparations: FrontstageNativePreparationSnapshot[];
  retryBlock(blockId: string): void;
}

export function useFrontstagePageCanvasNativePreparations({
  actorId,
  actorWorkspaceId,
  readPlan,
  catalogEntries,
  externalNpm,
  demandsByBlockId,
  maxConcurrent = 2,
  artifactCache = frontstageNativeReactArtifactCache,
  runtimeFingerprint,
  fetchSource = defaultFetchSource,
  compile = compileNativeReactComponentInBrowser,
  moduleRegistryFactory = createFrontstageNativeReactModuleRegistry
}: UseFrontstagePageCanvasNativePreparationsInput): UseFrontstagePageCanvasNativePreparationsResult {
  const scheduler = useMemo(
    () => new FrontstageNativePreparationScheduler(maxConcurrent),
    [maxConcurrent]
  );
  const [preparations, setPreparations] = useState<
    FrontstageNativePreparationSnapshot[]
  >([]);
  const componentFactoryFlights = useMemo(
    () =>
      new Map<
        string,
        ReturnType<typeof evaluateNativeReactComponentArtifactWithRegistry>
      >(),
    []
  );
  const moduleRegistryCache = useMemo(
    () => new FrontstagePageNativeModuleRegistryCache(),
    []
  );
  useEffect(
    () => scheduler.subscribe(() => setPreparations(scheduler.getSnapshots())),
    [scheduler]
  );

  const tasks = useMemo<FrontstageNativePreparationTask[]>(() => {
    if (
      !actorId ||
      !readPlan ||
      catalogEntries === null ||
      actorWorkspaceId !== readPlan.workspaceId
    ) {
      return [];
    }
    return readPlan.requests.map((request) => {
      const catalogEntry = catalogEntries?.find(
        (entry) =>
          entry.installationId === request.installationId &&
          entry.providerCode === request.providerCode &&
          entry.pluginId === request.pluginId &&
          entry.pluginVersion === request.pluginVersion &&
          entry.contributionCode === request.contributionCode
      );
      return {
        blockId: request.blockId,
        slotIndex: request.slotIndex,
        identity: [
          actorId,
          readPlan.workspaceId,
          request.codeRef,
          externalNpm.status,
          catalogEntries === undefined
            ? 'legacy-fixture'
            : catalogEntry
              ? JSON.stringify({
                  contributionId: catalogEntry.raw.frontend_contribution_id,
                  blockVersion: catalogEntry.raw.frontend_block_version,
                  graphFingerprint: catalogEntry.raw.graph_fingerprint,
                  grantedPermissions: catalogEntry.raw.granted_permissions,
                  assets: catalogEntry.raw.code_modules.flatMap((module) =>
                    module.assets.map((asset) => ({
                      sha256: asset.sha256,
                      url: asset.url,
                      integrity:
                        'integrity' in asset ? asset.integrity : 'external'
                    }))
                  )
                })
              : 'binding-missing'
        ].join('/'),
        observationContext: {
          actorId,
          workspaceId: readPlan.workspaceId,
          pageId: readPlan.pageId,
          tabId: null,
          blockId: request.blockId
        },
        observe: (observation) =>
          recordFrontstageRuntimeObservation({
            actorId,
            workspaceId: readPlan.workspaceId,
            pageId: readPlan.pageId,
            tabId: null,
            blockId: request.blockId,
            runtimeKind: 'native',
            ...observation
          }),
        prepare: async (signal, enterStage) => {
          const contribution =
            catalogEntries === undefined
              ? undefined
              : prepareFrontstageNativeContribution(
                  catalogEntries,
                  request,
                  readPlan.workspaceId
                );
          const source = await fetchSource(request, signal);
          throwIfAborted(signal);
          const executable = readLockedNativeReactExecutableStyle(source);
          const dependencyLock = executable.dependency_lock;
          const currentRuntimeFingerprint =
            runtimeFingerprint ??
            getNativeReactRuntimeFingerprint(
              dependencyLock,
              executable.executable_style_identity
            );
          const dependencyLockIdentity =
            nativeReactCatalogDependencyLockIdentity(dependencyLock);
          const legacyDiagnostic = diagnoseLegacyBlockModuleSource(
            executable.source_code
          );
          if (legacyDiagnostic) {
            throw new NativeReactSourceContractError(legacyDiagnostic);
          }
          enterStage('artifact_lookup');
          const identity = createFrontstageNativeReactArtifactCacheIdentity({
            actorId,
            workspaceId: readPlan.workspaceId,
            source: executable.source_code,
            dependencyLock,
            runtimeFingerprint: currentRuntimeFingerprint
          });
          const cached = await artifactCache.get(identity);
          throwIfAborted(signal);
          let artifact;
          let artifactCacheTier: 'l2' | 'miss';
          if (cached.status === 'hit') {
            artifact = cached.artifact;
            artifactCacheTier = 'l2';
          } else {
            enterStage('compile', 'miss');
            const compiled = await compile({
              source: executable.source_code,
              requestId: `${request.requestId}:${identity.source_sha256}`,
              dependencyLock,
              runtimeFingerprint: currentRuntimeFingerprint
            });
            throwIfAborted(signal);
            if (!compiled.ok) {
              throw new Error(
                describeExternalNpmImportFailure(
                  compiled.diagnostics[0]?.message ??
                    'Native React component compilation failed.',
                  externalNpm
                )
              );
            }
            artifact = compiled.artifact;
            artifactCacheTier = 'miss';
            await artifactCache.put(identity, artifact);
            throwIfAborted(signal);
          }

          enterStage('module_resolve', artifactCacheTier);
          const componentFactoryKey = JSON.stringify(artifact.identity);
          const moduleRegistry = moduleRegistryCache.get(
            dependencyLock,
            moduleRegistryFactory
          );
          let componentFactoryFlight =
            componentFactoryFlights.get(componentFactoryKey);
          if (!componentFactoryFlight) {
            componentFactoryFlight =
              evaluateNativeReactComponentArtifactWithRegistry(
                artifact,
                moduleRegistry
              );
            componentFactoryFlights.set(
              componentFactoryKey,
              componentFactoryFlight
            );
          }
          const evaluated = await componentFactoryFlight;
          throwIfAborted(signal);
          if (!evaluated.ok) {
            componentFactoryFlights.delete(componentFactoryKey);
            throw new Error(
              evaluated.diagnostics[0]?.message ??
                'Native React module resolution failed.'
            );
          }
          const moduleAssets = await moduleRegistry.resolveModuleAssets(
            evaluated.artifact.program.injectedModules.map(
              (module) => module.source
            )
          );
          throwIfAborted(signal);
          return {
            artifact: evaluated.artifact,
            component: evaluated.component,
            artifactCacheTier,
            moduleAssets: [...moduleAssets, executable.shadow_style_asset],
            generatedCssSha256: executable.generated_css_sha256,
            ...(contribution ? { contribution } : {}),
            identityInput: {
              sourceSha256: evaluated.artifact.identity.source_sha256,
              runtimeFingerprint: currentRuntimeFingerprint,
              dependencyLockIdentity,
              executableStyleIdentity: executable.executable_style_identity
            }
          };
        }
      };
    });
  }, [
    actorId,
    actorWorkspaceId,
    artifactCache,
    compile,
    catalogEntries,
    componentFactoryFlights,
    externalNpm,
    fetchSource,
    moduleRegistryFactory,
    moduleRegistryCache,
    readPlan,
    runtimeFingerprint
  ]);

  useEffect(() => {
    scheduler.reconcile(tasks, demandsByBlockId);
  }, [demandsByBlockId, scheduler, tasks]);

  useEffect(() => {
    if (typeof document === 'undefined') return;
    const updateVisibility = () =>
      scheduler.setPageVisible(document.visibilityState !== 'hidden');
    updateVisibility();
    document.addEventListener('visibilitychange', updateVisibility);
    return () =>
      document.removeEventListener('visibilitychange', updateVisibility);
  }, [scheduler]);

  useEffect(() => () => scheduler.dispose(), [scheduler]);

  const retryBlock = useCallback(
    (blockId: string) => scheduler.retry(blockId),
    [scheduler]
  );
  return { preparations, retryBlock };
}

async function defaultFetchSource(
  request: FrontstagePageCanvasBlockCodeReadPlan['requests'][number],
  signal: AbortSignal
): Promise<NativePreparationSource> {
  throwIfAborted(signal);
  const source = await getFrontstageBlockCode(
    request.workspaceId,
    request.pageId,
    request.codeRef,
    getFrontstageApiBaseUrl()
  );
  throwIfAborted(signal);
  return source;
}

function throwIfAborted(signal: AbortSignal): void {
  if (signal.aborted)
    throw new DOMException('Preparation aborted.', 'AbortError');
}
