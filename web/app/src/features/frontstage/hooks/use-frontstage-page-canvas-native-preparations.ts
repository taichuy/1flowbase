import { evaluateFrontstageReactArtifact } from '../lib/native-modules/modal/evaluator';
import { subscribeFrontstageSourceChanges } from '../lib/runtime-cache/source-changes';
import {
  diagnoseLegacyBlockModuleSource,
  NativeReactSourceContractError,
  sha256Text,
  type NativeReactModuleDefinition,
  type NativeReactModuleRegistry
} from '@1flowbase/page-runtime';
import {
  revalidateConsoleFrontstageBlockNodeCode,
  type ConsoleFrontstageBlockNodeCode
} from '@1flowbase/api-client';
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState
} from 'react';

import { getFrontstageApiBaseUrl } from '../api/page-tree';
import {
  compileNativeReactComponentInBrowser,
  type NativeReactBrowserCompileResult
} from '../../../shared/code-block/native-react-compiler-browser';
import { createFrontstageNativeReactModuleRegistry } from '../lib/native-modules/registry';
import {
  createFrontstageNativeReactArtifactCacheIdentity,
  createFrontstageNativeReactArtifactCacheKey,
  frontstageNativeReactArtifactCache,
  type FrontstageNativeReactArtifactCache
} from '../lib/runtime-cache';
import {
  FrontstageNativePreparationScheduler,
  FrontstageNativeSourceReadError,
  prepareFrontstageNativeContribution,
  type FrontstageNativePreparedRuntime,
  type FrontstageNativePreparationSource,
  type FrontstageNativePreparationTask
} from '../lib/page-canvas/native-runtime-preparation';
import type { FrontstagePageCanvasBlockCodeReadPlan } from '../lib/page-canvas/runtime-source';
import type { FrontstageRuntimeDemandByBlockId } from '../lib/page-canvas/runtime-demand';
import { recordFrontstageRuntimeObservation } from '../lib/page-canvas/runtime-observation';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';

type NativePreparationSource = ConsoleFrontstageBlockNodeCode;
type NativeComponentEvaluation = Awaited<
  ReturnType<typeof evaluateFrontstageReactArtifact>
>;
type NativeComponentFlight =
  | {
      ok: true;
      evaluated: Extract<NativeComponentEvaluation, { ok: true }>;
      moduleAssets: Awaited<
        ReturnType<NativeReactModuleRegistry['resolveModuleAssets']>
      >;
      moduleSources: string[];
    }
  | {
      ok: false;
      evaluated: Extract<NativeComponentEvaluation, { ok: false }>;
    };

export interface UseFrontstagePageCanvasNativePreparationsInput {
  active?: boolean;
  actorId: string | null | undefined;
  actorWorkspaceId: string | null | undefined;
  readPlan: FrontstagePageCanvasBlockCodeReadPlan | null | undefined;
  catalogEntries?: readonly NormalizedFrontstageBlockCatalogEntry[] | null;
  demandsByBlockId?: FrontstageRuntimeDemandByBlockId;
  maxConcurrent?: number;
  artifactCache?: Pick<FrontstageNativeReactArtifactCache, 'get' | 'put'> &
    Partial<Pick<FrontstageNativeReactArtifactCache, 'captureWriteEpoch'>>;
  fetchSource?: (
    request: FrontstagePageCanvasBlockCodeReadPlan['requests'][number],
    signal: AbortSignal,
    cached?: NativePreparationSource
  ) => Promise<NativePreparationSource>;
  compile?: (input: {
    source: string;
    requestId: string;
    moduleDefinitions: readonly NativeReactModuleDefinition[];
    signal?: AbortSignal;
    flightKey?: string;
  }) => Promise<NativeReactBrowserCompileResult>;
  moduleRegistryFactory?: () => NativeReactModuleRegistry;
}

export interface UseFrontstagePageCanvasNativePreparationsResult {
  isValidating: boolean;
  validationError: Error | null;
  retryValidation(): void;
  preparations: FrontstageNativePreparationSource;
  noteInteraction(): void;
  retryBlock(blockId: string): void;
  refreshBlock(blockId: string): void;
}

export function useFrontstagePageCanvasNativePreparations({
  active = true,
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
    [maxConcurrent, actorId, actorWorkspaceId]
  );
  const wasActive = useRef(active);
  const needsReturnValidation = active && !wasActive.current;
  const [isValidating, setIsValidating] = useState(false);
  const [validationError, setValidationError] = useState<Error | null>(null);
  const [validationAttempt, setValidationAttempt] = useState(0);
  const sourceCopies = useMemo(
    () => new Map<string, NativePreparationSource>(),
    [scheduler]
  );
  const compiledRefreshes = useMemo(
    () => new Map<string, number>(),
    [scheduler]
  );
  const [refreshGenerationsByRequestId, setRefreshGenerationsByRequestId] =
    useState<Record<string, number>>({});
  const moduleDefinitions = useMemo(
    () => moduleRegistryFactory().definitions,
    [moduleRegistryFactory, actorId, actorWorkspaceId]
  );
  const modulePolicySha256 = useMemo(
    () =>
      sha256Text(
        JSON.stringify(
          moduleDefinitions
            .map((definition) => ({
              module_source: definition.module_source,
              exports: [...definition.exports].sort()
            }))
            .sort((left, right) =>
              left.module_source < right.module_source
                ? -1
                : left.module_source > right.module_source
                  ? 1
                  : 0
            )
        )
      ),
    [moduleDefinitions]
  );
  const componentFactoryFlights = useMemo(
    () => new Map<string, Promise<NativeComponentFlight>>(),
    [actorId, actorWorkspaceId, moduleRegistryFactory, modulePolicySha256]
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
      let preparedCopy: FrontstageNativePreparedRuntime | undefined;
      let preparedSource: NativePreparationSource | undefined;
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
        explicitRefresh: forceCompile,
        identity: [
          actorId,
          readPlan.workspaceId,
          request.codeRef,
          modulePolicySha256,
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
          const forceCompile =
            refreshGeneration > (compiledRefreshes.get(request.requestId) ?? 0);
          const contribution =
            catalogEntries === undefined
              ? undefined
              : prepareFrontstageNativeContribution(
                  catalogEntries,
                  request,
                  readPlan.workspaceId
                );
          const writeEpoch = artifactCache
            .captureWriteEpoch?.(actorId)
            .catch(() => ({ local: -1, persisted: -1 }));
          let source: NativePreparationSource;
          try {
            source = await fetchSource(
              request,
              signal,
              forceCompile ? undefined : sourceCopies.get(request.requestId)
            );
          } catch (error) {
            throw new FrontstageNativeSourceReadError(error);
          }
          throwIfAborted(signal);
          // A conditional 304 read returns the same validated browser copy.
          // ACL was checked by the backend; the mounted factory needs no work.
          if (!forceCompile && source === preparedSource && preparedCopy)
            return preparedCopy;
          // The backend digest is the conditional-read contract. Reject corrupt
          // source before it can become a trusted browser copy or artifact.
          if (
            source.source_sha256 &&
            sha256Text(source.source_code) !== source.source_sha256
          ) {
            sourceCopies.delete(request.requestId);
            throw new Error(
              'Native React source revision does not match its content.'
            );
          }
          sourceCopies.set(request.requestId, source);
          const legacyDiagnostic = diagnoseLegacyBlockModuleSource(
            source.source_code
          );
          if (legacyDiagnostic) {
            throw new NativeReactSourceContractError(legacyDiagnostic);
          }
          const identity = createFrontstageNativeReactArtifactCacheIdentity({
            actorId,
            workspaceId: readPlan.workspaceId,
            source: source.source_code,
            modulePolicySha256
          });
          let artifact;
          let artifactCacheTier: 'l1' | 'l2' | 'miss' = 'miss';
          if (!forceCompile) {
            await enterStage('artifact_lookup');
            const cached = await artifactCache.get(identity);
            throwIfAborted(signal);
            if (cached.status === 'hit') {
              artifact = cached.artifact;
              artifactCacheTier = cached.tier ?? 'l2';
            }
          }
          if (!artifact) {
            await enterStage('compile', 'miss');
            const compiled = await compile({
              source: source.source_code,
              requestId: `${request.requestId}:${identity.source_sha256}:refresh:${refreshGeneration}`,
              moduleDefinitions,
              signal,
              ...(forceCompile
                ? {}
                : {
                    flightKey:
                      createFrontstageNativeReactArtifactCacheKey(identity)
                  })
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
            void artifactCache
              .put(identity, artifact, writeEpoch)
              .catch(() => undefined);
          }

          await enterStage('module_resolve', artifactCacheTier);
          const componentFactoryKey =
            createFrontstageNativeReactArtifactCacheKey(identity);
          if (forceCompile) componentFactoryFlights.delete(componentFactoryKey);
          let componentFactoryFlight =
            componentFactoryFlights.get(componentFactoryKey);
          if (!componentFactoryFlight) {
            // Evaluated module facades (including generated styles) belong to this factory.
            const moduleRegistry = moduleRegistryFactory();
            componentFactoryFlight =
              (async (): Promise<NativeComponentFlight> => {
                const evaluated = await evaluateFrontstageReactArtifact(
                  artifact,
                  moduleRegistry
                );
                if (!evaluated.ok) return { ok: false, evaluated };
                const moduleSources =
                  evaluated.artifact.program.injectedModules.map(
                    (module) => module.source
                  );
                const moduleAssets =
                  await moduleRegistry.resolveModuleAssets(moduleSources);
                return {
                  ok: true,
                  evaluated,
                  moduleAssets,
                  moduleSources
                };
              })();
            componentFactoryFlights.set(
              componentFactoryKey,
              componentFactoryFlight
            );
          }
          let componentFlight;
          try {
            componentFlight = await componentFactoryFlight;
          } catch (error) {
            if (
              componentFactoryFlights.get(componentFactoryKey) ===
              componentFactoryFlight
            )
              componentFactoryFlights.delete(componentFactoryKey);
            throw error;
          }
          throwIfAborted(signal);
          if (!componentFlight.ok) {
            if (
              componentFactoryFlights.get(componentFactoryKey) ===
              componentFactoryFlight
            )
              componentFactoryFlights.delete(componentFactoryKey);
            throw new Error(
              componentFlight.evaluated.diagnostics[0]?.message ??
                'Native React module resolution failed.'
            );
          }
          const { evaluated, moduleAssets, moduleSources } = componentFlight;
          throwIfAborted(signal);
          compiledRefreshes.set(request.requestId, refreshGeneration);
          preparedCopy = {
            artifact: evaluated.artifact,
            component: evaluated.component,
            artifactCacheTier,
            moduleAssets,
            moduleSources,
            ...(contribution ? { contribution } : {}),
            identityInput: {
              sourceSha256: evaluated.artifact.identity.source_sha256,
              compilerAbi: evaluated.artifact.identity.compiler_abi,
              runtimeAbi: evaluated.artifact.identity.runtime_abi
            }
          };
          preparedSource = source;
          return preparedCopy;
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
    moduleDefinitions,
    moduleRegistryFactory,
    modulePolicySha256,
    readPlan,
    refreshGenerationsByRequestId,
    sourceCopies,
    compiledRefreshes
  ]);

  useLayoutEffect(() => {
    if (!active) scheduler.suspend();
    scheduler.reconcile(tasks, demandsByBlockId);
  }, [active, demandsByBlockId, scheduler, tasks]);

  useLayoutEffect(() => {
    let current = true;
    const returning = active && !wasActive.current;
    wasActive.current = active;
    if (!active) {
      scheduler.suspend();
      setIsValidating(false);
    } else if (returning || validationAttempt > 0) {
      scheduler.suspend();
      setIsValidating(true);
      setValidationError(null);
      void scheduler.revalidate().then((error) => {
        if (!current) return;
        setValidationError(error);
        setIsValidating(false);
        scheduler.setPageVisible(
          !error &&
            (typeof document === 'undefined' ||
              document.visibilityState !== 'hidden')
        );
      });
    }
    return () => {
      current = false;
    };
  }, [active, scheduler, validationAttempt]);

  useEffect(() => {
    if (typeof document === 'undefined') return;
    const updateVisibility = () =>
      scheduler.setPageVisible(
        active &&
          !isValidating &&
          !validationError &&
          document.visibilityState !== 'hidden'
      );
    updateVisibility();
    document.addEventListener('visibilitychange', updateVisibility);
    return () =>
      document.removeEventListener('visibilitychange', updateVisibility);
  }, [active, isValidating, validationError, scheduler]);

  useEffect(() => () => scheduler.dispose(), [scheduler]);

  useEffect(
    () =>
      subscribeFrontstageSourceChanges((change) => {
        if (
          change.workspaceId !== readPlan?.workspaceId ||
          change.pageId !== readPlan.pageId
        )
          return;
        const request = readPlan.requests.find(
          (entry) => entry.blockId === change.blockId
        );
        if (request) sourceCopies.delete(request.requestId);
        scheduler.retry(change.blockId);
      }),
    [readPlan, scheduler, sourceCopies]
  );

  const retryBlock = useCallback(
    (blockId: string) => scheduler.retry(blockId),
    [scheduler]
  );
  const retryValidation = useCallback(
    () => setValidationAttempt((attempt) => attempt + 1),
    []
  );
  const noteInteraction = useCallback(
    () => scheduler.noteInteraction(),
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
  return {
    preparations: scheduler,
    isValidating: isValidating || needsReturnValidation,
    validationError,
    retryValidation,
    noteInteraction,
    retryBlock,
    refreshBlock
  };
}

async function defaultFetchSource(
  request: FrontstagePageCanvasBlockCodeReadPlan['requests'][number],
  signal: AbortSignal,
  cached?: NativePreparationSource
): Promise<NativePreparationSource> {
  throwIfAborted(signal);
  const response = await revalidateConsoleFrontstageBlockNodeCode(
    request.pageId,
    request.blockId,
    {
      baseUrl: getFrontstageApiBaseUrl(),
      signal,
      source_sha256: cached?.source_sha256
    }
  );
  throwIfAborted(signal);
  if (response.status === 'modified') return response.value;
  if (cached?.source_sha256) return cached;
  throw new Error('Source revalidation returned no browser copy.');
}

function throwIfAborted(signal: AbortSignal): void {
  if (signal.aborted)
    throw new DOMException('Preparation aborted.', 'AbortError');
}
