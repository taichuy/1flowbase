import {
  evaluateNativeReactComponentArtifactWithRegistry,
  nativeReactCatalogDependencyLockIdentity,
  type NativeReactCatalogDependencyLock,
  type NativeReactModuleRegistry
} from '@1flowbase/page-runtime';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { fetchFrontstageBlockCode } from '../api/block-code';
import {
  compileNativeReactComponentInBrowser,
  getNativeReactRuntimeFingerprint,
  type NativeReactBrowserCompileResult
} from '../../../shared/code-block/native-react-compiler-browser';
import { createFrontstageNativeReactModuleRegistry } from '../lib/native-trusted-block-runtime-factory';
import {
  createFrontstageNativeReactArtifactCacheIdentity,
  frontstageNativeReactArtifactCache,
  type FrontstageNativeReactArtifactCache
} from '../lib/runtime-cache';
import {
  FrontstageNativePreparationScheduler,
  type FrontstageNativePreparationSnapshot,
  type FrontstageNativePreparationTask
} from '../lib/page-canvas/native-runtime-preparation';
import {
  frontstageRuntimeSourceMatchesDigest,
  type FrontstagePageCanvasBlockCodeReadPlan
} from '../lib/page-canvas/runtime-source';
import type { FrontstageRuntimeDemandByBlockId } from '../lib/page-canvas/runtime-demand';

interface NativePreparationSource {
  code: string;
  source_sha256: string;
}

export interface UseFrontstagePageCanvasNativePreparationsInput {
  actorId: string | null | undefined;
  actorWorkspaceId: string | null | undefined;
  readPlan: FrontstagePageCanvasBlockCodeReadPlan | null | undefined;
  dependencyLocksByBlockId: Readonly<
    Record<string, NativeReactCatalogDependencyLock>
  >;
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
  dependencyLocksByBlockId,
  demandsByBlockId,
  maxConcurrent = 2,
  artifactCache = frontstageNativeReactArtifactCache,
  runtimeFingerprint = getNativeReactRuntimeFingerprint(),
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
  useEffect(
    () => scheduler.subscribe(() => setPreparations(scheduler.getSnapshots())),
    [scheduler]
  );

  const tasks = useMemo<FrontstageNativePreparationTask[]>(() => {
    if (!actorId || !readPlan || actorWorkspaceId !== readPlan.workspaceId) {
      return [];
    }
    return readPlan.requests.map((request) => {
      const dependencyLock = dependencyLocksByBlockId[request.blockId] ?? [];
      const dependencyLockIdentity =
        nativeReactCatalogDependencyLockIdentity(dependencyLock);
      return {
        blockId: request.blockId,
        slotIndex: request.slotIndex,
        identity: [
          actorId,
          readPlan.workspaceId,
          request.codeRef,
          runtimeFingerprint,
          dependencyLockIdentity
        ].join('/'),
        prepare: async (signal, enterStage) => {
          const source = await fetchSource(request, signal);
          throwIfAborted(signal);
          if (
            !frontstageRuntimeSourceMatchesDigest(
              source.code,
              source.source_sha256
            )
          ) {
            throw new Error(
              `Block code digest does not match source_sha256 for ${request.codeRef}.`
            );
          }
          enterStage('artifact_lookup');
          const identity = createFrontstageNativeReactArtifactCacheIdentity({
            actorId,
            workspaceId: readPlan.workspaceId,
            source: source.code,
            dependencyLock,
            runtimeFingerprint
          });
          const cached = await artifactCache.get(identity);
          throwIfAborted(signal);
          let artifact;
          let artifactCacheTier: 'l2' | 'miss';
          if (cached.status === 'hit') {
            artifact = cached.artifact;
            artifactCacheTier = 'l2';
          } else {
            enterStage('compile');
            const compiled = await compile({
              source: source.code,
              requestId: `${request.requestId}:${identity.source_sha256}`,
              dependencyLock,
              runtimeFingerprint
            });
            throwIfAborted(signal);
            if (!compiled.ok) {
              throw new Error(
                compiled.diagnostics[0]?.message ??
                  'Native React component compilation failed.'
              );
            }
            artifact = compiled.artifact;
            artifactCacheTier = 'miss';
            await artifactCache.put(identity, artifact);
            throwIfAborted(signal);
          }

          enterStage('module_resolve');
          const evaluated =
            await evaluateNativeReactComponentArtifactWithRegistry(
              artifact,
              moduleRegistryFactory(dependencyLock)
            );
          throwIfAborted(signal);
          if (!evaluated.ok) {
            throw new Error(
              evaluated.diagnostics[0]?.message ??
                'Native React module resolution failed.'
            );
          }
          return {
            artifact: evaluated.artifact,
            component: evaluated.component,
            artifactCacheTier,
            identityInput: {
              sourceSha256: evaluated.artifact.identity.source_sha256,
              runtimeFingerprint,
              dependencyLockIdentity
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
    dependencyLocksByBlockId,
    fetchSource,
    moduleRegistryFactory,
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
  const source = await fetchFrontstageBlockCode(
    request.workspaceId,
    request.pageId,
    request.codeRef
  );
  throwIfAborted(signal);
  if (!source.code?.trim() || !source.source_sha256?.trim()) {
    throw new Error(`Block code is empty for ${request.codeRef}.`);
  }
  return { code: source.code, source_sha256: source.source_sha256 };
}

function throwIfAborted(signal: AbortSignal): void {
  if (signal.aborted)
    throw new DOMException('Preparation aborted.', 'AbortError');
}
