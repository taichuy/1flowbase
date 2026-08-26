import {
  evaluateNativeReactComponentArtifactWithRegistry,
  diagnoseLegacyBlockModuleSource,
  NativeReactSourceContractError,
  sha256Text,
  type NativeReactModuleDefinition,
  type NativeReactModuleRegistry
} from '@1flowbase/page-runtime';
import {
  getConsoleFrontstageBlockNodeCode,
  type ConsoleFrontstageBlockNodeCode
} from '@1flowbase/api-client';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { getFrontstageApiBaseUrl } from '../api/page-tree';
import {
  compileNativeReactComponentInBrowser,
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
  prepareFrontstageNativeContribution,
  type FrontstageNativePreparationSnapshot,
  type FrontstageNativePreparationTask
} from '../lib/page-canvas/native-runtime-preparation';
import type { FrontstagePageCanvasBlockCodeReadPlan } from '../lib/page-canvas/runtime-source';
import type { FrontstageRuntimeDemandByBlockId } from '../lib/page-canvas/runtime-demand';
import { recordFrontstageRuntimeObservation } from '../lib/page-canvas/runtime-observation';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';

type NativePreparationSource = ConsoleFrontstageBlockNodeCode;

export interface UseFrontstagePageCanvasNativePreparationsInput {
  actorId: string | null | undefined;
  actorWorkspaceId: string | null | undefined;
  readPlan: FrontstagePageCanvasBlockCodeReadPlan | null | undefined;
  catalogEntries?: readonly NormalizedFrontstageBlockCatalogEntry[] | null;
  demandsByBlockId?: FrontstageRuntimeDemandByBlockId;
  maxConcurrent?: number;
  artifactCache?: Pick<FrontstageNativeReactArtifactCache, 'get' | 'put'>;
  fetchSource?: (
    request: FrontstagePageCanvasBlockCodeReadPlan['requests'][number],
    signal: AbortSignal
  ) => Promise<NativePreparationSource>;
  compile?: (input: {
    source: string;
    requestId: string;
    moduleDefinitions: readonly NativeReactModuleDefinition[];
  }) => Promise<NativeReactBrowserCompileResult>;
  moduleRegistryFactory?: () => NativeReactModuleRegistry;
}

export interface UseFrontstagePageCanvasNativePreparationsResult {
  preparations: FrontstageNativePreparationSnapshot[];
  retryBlock(blockId: string): void;
  refreshBlock(blockId: string): void;
}

export function useFrontstagePageCanvasNativePreparations({
  actorId,
  actorWorkspaceId,
  readPlan,
  catalogEntries,
  demandsByBlockId,
  maxConcurrent = 2,
  artifactCache = frontstageNativeReactArtifactCache,
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
  const [refreshGenerationsByRequestId, setRefreshGenerationsByRequestId] =
    useState<Record<string, number>>({});
  const componentFactoryFlights = useMemo(
    () =>
      new Map<
        string,
        ReturnType<typeof evaluateNativeReactComponentArtifactWithRegistry>
      >(),
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
      const refreshGeneration =
        refreshGenerationsByRequestId[request.requestId] ?? 0;
      const forceCompile = refreshGeneration > 0;
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
          `refresh:${refreshGeneration}`,
          catalogEntries === undefined
            ? 'legacy-fixture'
            : catalogEntry
              ? JSON.stringify({
                  contributionId: catalogEntry.raw.frontend_contribution_id,
                  blockVersion: catalogEntry.raw.frontend_block_version,
                  graphFingerprint: catalogEntry.raw.graph_fingerprint,
                  grantedPermissions: catalogEntry.raw.granted_permissions
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
          const sourceIdentity = {
            sourceSha256: source.source_sha256 ?? sha256Text(source.source_code),
            compilerAbi: '1flowbase/unrestricted-tsx@1',
            runtimeAbi: '1flowbase/unrestricted-iframe@1'
          };
          if (hasUnregisteredBrowserImport(source.source_code, moduleRegistryFactory)) {
            return {
              source: source.source_code,
              artifactCacheTier: 'miss',
              identityInput: sourceIdentity,
              moduleAssets: []
            };
          }
          const legacyDiagnostic = diagnoseLegacyBlockModuleSource(
            source.source_code
          );
          if (legacyDiagnostic) {
            throw new NativeReactSourceContractError(legacyDiagnostic);
          }
          const identity = createFrontstageNativeReactArtifactCacheIdentity({
            actorId,
            workspaceId: readPlan.workspaceId,
            source: source.source_code
          });
          let artifact;
          let artifactCacheTier: 'l2' | 'miss' = 'miss';
          if (!forceCompile) {
            enterStage('artifact_lookup');
            const cached = await artifactCache.get(identity);
            throwIfAborted(signal);
            if (cached.status === 'hit') {
              artifact = cached.artifact;
              artifactCacheTier = 'l2';
            }
          }
          if (!artifact) {
            enterStage('compile', 'miss');
            const moduleRegistry = moduleRegistryFactory();
            const compiled = await compile({
              source: source.source_code,
              requestId: `${request.requestId}:${identity.source_sha256}`,
              moduleDefinitions: moduleRegistry.definitions
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

          enterStage('module_resolve', artifactCacheTier);
          const componentFactoryKey = JSON.stringify(artifact.identity);
          if (forceCompile) componentFactoryFlights.delete(componentFactoryKey);
          const moduleRegistry = moduleRegistryFactory();
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
            source: source.source_code,
            artifact: evaluated.artifact,
            component: evaluated.component,
            artifactCacheTier,
            moduleAssets,
            ...(contribution ? { contribution } : {}),
            identityInput: {
              sourceSha256: evaluated.artifact.identity.source_sha256,
              compilerAbi: evaluated.artifact.identity.compiler_abi,
              runtimeAbi: evaluated.artifact.identity.runtime_abi
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
    fetchSource,
    moduleRegistryFactory,
    readPlan,
    refreshGenerationsByRequestId
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
  const refreshBlock = useCallback(
    (blockId: string) => {
      const request = readPlan?.requests.find(
        (candidate) => candidate.blockId === blockId
      );
      if (!request) return;
      setRefreshGenerationsByRequestId((current) => ({
        ...current,
        [request.requestId]: (current[request.requestId] ?? 0) + 1
      }));
    },
    [readPlan]
  );
  return { preparations, retryBlock, refreshBlock };
}

function hasUnregisteredBrowserImport(
  source: string,
  moduleRegistryFactory: () => NativeReactModuleRegistry
): boolean {
  const registeredSources = new Set(
    moduleRegistryFactory().definitions.map(({ module_source }) => module_source)
  );
  const importPattern = /\b(?:from\s*|import\s*)(['"])([^'"\n]+)\1/gu;
  return [...source.matchAll(importPattern)].some(
    (match) => !registeredSources.has(match[2])
  );
}

async function defaultFetchSource(
  request: FrontstagePageCanvasBlockCodeReadPlan['requests'][number],
  signal: AbortSignal
): Promise<NativePreparationSource> {
  throwIfAborted(signal);
  const source = await getConsoleFrontstageBlockNodeCode(
    request.pageId,
    request.blockId,
    getFrontstageApiBaseUrl()
  );
  throwIfAborted(signal);
  return source;
}

function throwIfAborted(signal: AbortSignal): void {
  if (signal.aborted)
    throw new DOMException('Preparation aborted.', 'AbortError');
}
