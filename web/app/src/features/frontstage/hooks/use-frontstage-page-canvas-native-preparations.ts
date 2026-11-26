import { evaluateFrontstageReactArtifact } from '../lib/native-modules/modal/evaluator';
import { subscribeFrontstageSourceChanges } from '../lib/runtime-cache/source-changes';
import {
  diagnoseLegacyBlockModuleSource,
  NativeReactSourceContractError,
  sha256Text,
  type NativeReactModuleDefinition,
  type NativeReactModuleRegistry
} from '@1flowbase/page-runtime/browser';
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
  promoteDefaultNativeReactCompilerFlight,
  type NativeReactBrowserCompileResult
} from '../../../shared/code-block/native-react-compiler-browser';
import {
  createFrontstageNativeReactModuleRegistry,
  prewarmNativeModuleSources
} from '../lib/native-modules/registry';
import {
  consumeStartupPrefetchedBlockNodeCode,
  invalidateStartupPrefetchedBlockNodeCode,
  readStartupPrefetchedBlockNodeCode
} from '../api/startup-block-code-cache';
import { createContributionModuleRegistry } from '../lib/native-modules/contributions/registry';
import {
  createFrontstageNativeReactArtifactCacheIdentity,
  createFrontstageNativeReactArtifactCacheIdentityFromDigest,
  createFrontstageNativeReactArtifactCacheKey,
  type FrontstageNativeReactArtifactCache,
  type FrontstageNativeReactArtifactCacheIdentity
} from '../lib/runtime-cache/native-react-artifact-cache';
import type { FrontstageNativeBlockSourceCache } from '../lib/runtime-cache/native-block-source-cache';
import {
  frontstageNativeBlockSourceCache,
  frontstageNativeReactArtifactCache
} from '../lib/runtime-cache/default-cache';
import {
  DEFAULT_FRONTSTAGE_NATIVE_PREPARATION_CONCURRENCY,
  FrontstageNativePreparationScheduler,
  FrontstageNativeSourceReadError,
  prepareFrontstageNativeContribution,
  type FrontstageNativePreparedRuntime,
  type FrontstageNativePreparationSource,
  type FrontstageNativePreparationTask
} from '../lib/page-canvas/native-runtime-preparation';
import type { FrontstagePageCanvasBlockCodeReadPlan } from '../lib/page-canvas/runtime-source';
import {
  createFrontstageRuntimeDemandCandidates,
  resolveFrontstageRuntimeDemand,
  type FrontstageRuntimeDemandByBlockId
} from '../lib/page-canvas/runtime-demand';
import { recordFrontstageRuntimeObservation } from '../lib/page-canvas/runtime-observation';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import { resolveRuntimeConcurrencyProfile } from '../../../shared/utils/runtime-concurrency-profile';

type NativePreparationSource = ConsoleFrontstageBlockNodeCode;
type NativePreparationSourceCache = Pick<
  FrontstageNativeBlockSourceCache,
  'get' | 'put' | 'delete'
> &
  Partial<Pick<FrontstageNativeBlockSourceCache, 'captureWriteEpoch'>>;

interface PrewarmSourceFlight {
  controller: AbortController;
  promise: Promise<NativePreparationSource>;
}
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
  sourceCache?: NativePreparationSourceCache | null;
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
    priority?: number;
    slotIndex?: number;
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
  maxConcurrent = resolveRuntimeConcurrencyProfile()
    .canvasPreparationConcurrency ||
    DEFAULT_FRONTSTAGE_NATIVE_PREPARATION_CONCURRENCY,
  artifactCache = frontstageNativeReactArtifactCache,
  sourceCache,
  fetchSource = defaultFetchSource,
  compile = compileNativeReactComponentInBrowser,
  moduleRegistryFactory = createFrontstageNativeReactModuleRegistry
}: UseFrontstagePageCanvasNativePreparationsInput): UseFrontstagePageCanvasNativePreparationsResult {
  const resolvedSourceCache =
    sourceCache !== undefined
      ? sourceCache
      : fetchSource === defaultFetchSource
        ? frontstageNativeBlockSourceCache
        : null;
  const preparationScope = useMemo(
    () => ({
      actorId,
      actorWorkspaceId,
      scheduler: new FrontstageNativePreparationScheduler(maxConcurrent),
      sourceCopies: new Map<string, NativePreparationSource>(),
      prewarmFlights: new Map<string, PrewarmSourceFlight>(),
      compiledRefreshes: new Map<string, number>(),
      activeCompileFlightKeys: new Map<string, string>()
    }),
    [actorId, actorWorkspaceId, maxConcurrent]
  );
  const {
    scheduler,
    sourceCopies,
    prewarmFlights,
    compiledRefreshes,
    activeCompileFlightKeys
  } = preparationScope;
  const moduleScope = useMemo(() => {
    const hasFetchedModules = catalogEntries?.some(
      (entry) => entry.raw.code_modules.length
    );
    let loader = hasFetchedModules
      ? createContributionModuleRegistry(catalogEntries!, moduleRegistryFactory)
      : null;
    const registry = loader?.registry ?? moduleRegistryFactory();
    const moduleDefinitions = registry.definitions;
    return {
      actorId: preparationScope.actorId,
      actorWorkspaceId: preparationScope.actorWorkspaceId,
      moduleDefinitions,
      createEvaluationRegistry: () => {
        if (!hasFetchedModules) return moduleRegistryFactory();
        loader ??= createContributionModuleRegistry(
          catalogEntries!,
          moduleRegistryFactory
        );
        return loader.registry;
      },
      dispose: () => {
        loader?.dispose();
        loader = null;
      },
      modulePolicySha256: sha256Text(
        JSON.stringify({
          modules: loader?.policy ?? [],
          definitions: moduleDefinitions
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
        })
      ),
      componentFactoryFlights: new Map<string, Promise<NativeComponentFlight>>()
    };
  }, [
    preparationScope.actorId,
    preparationScope.actorWorkspaceId,
    moduleRegistryFactory,
    catalogEntries
  ]);
  useEffect(() => () => moduleScope.dispose(), [moduleScope]);
  const {
    actorId: scopedActorId,
    actorWorkspaceId: scopedWorkspaceId,
    moduleDefinitions,
    createEvaluationRegistry,
    modulePolicySha256,
    componentFactoryFlights
  } = moduleScope;
  const wasActive = useRef(active);
  const demandsRef = useRef(demandsByBlockId);
  demandsRef.current = demandsByBlockId;
  const needsReturnValidation = active && !wasActive.current;
  const [isValidating, setIsValidating] = useState(false);
  const [validationError, setValidationError] = useState<Error | null>(null);
  const [validationAttempt, setValidationAttempt] = useState(0);
  const [refreshGenerationsByRequestId, setRefreshGenerationsByRequestId] =
    useState<Record<string, number>>({});
  const tasks = useMemo<FrontstageNativePreparationTask[]>(() => {
    if (
      !scopedActorId ||
      !readPlan ||
      catalogEntries === null ||
      scopedWorkspaceId !== readPlan.workspaceId
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
          scopedActorId,
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
          actorId: scopedActorId,
          workspaceId: readPlan.workspaceId,
          pageId: readPlan.pageId,
          tabId: null,
          blockId: request.blockId
        },
        observe: (observation) =>
          recordFrontstageRuntimeObservation({
            actorId: scopedActorId,
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
          const prewarmed = !forceCompile
            ? prewarmFlights.get(request.requestId)
            : undefined;
          if (prewarmed) {
            prewarmFlights.delete(request.requestId);
          } else if (forceCompile) {
            const stalePrewarm = prewarmFlights.get(request.requestId);
            if (stalePrewarm) {
              stalePrewarm.controller.abort();
              prewarmFlights.delete(request.requestId);
            }
          }
          const contribution =
            catalogEntries === undefined
              ? undefined
              : prepareFrontstageNativeContribution(
                  catalogEntries,
                  request,
                  readPlan.workspaceId
                );
          const writeEpoch = artifactCache
            .captureWriteEpoch?.(scopedActorId)
            .catch(() => ({ local: -1, persisted: -1 }));
          const knownDigest =
            !forceCompile &&
            fetchSource === defaultFetchSource &&
            typeof request.source_sha256 === 'string' &&
            /^[0-9a-f]{64}$/iu.test(request.source_sha256.trim())
              ? request.source_sha256.trim().toLowerCase()
              : null;

          // Fast-path: when `bootstrap` already authorized code access and supplied `source_sha256`,
          // check L1/L2 `artifactCache` directly by digest before downloading `source_code` or
          // spawning the Compiler Worker.
          let identity: FrontstageNativeReactArtifactCacheIdentity | undefined;
          let artifact;
          let artifactCacheTier: 'l1' | 'l2' | 'miss' = 'miss';
          let source: ConsoleFrontstageBlockNodeCode | null = null;

          if (knownDigest) {
            if (
              preparedCopy &&
              preparedCopy.identityInput.sourceSha256 === knownDigest
            ) {
              return preparedCopy;
            }
            const digestIdentity =
              createFrontstageNativeReactArtifactCacheIdentityFromDigest({
                actorId: scopedActorId,
                workspaceId: readPlan.workspaceId,
                sourceSha256: knownDigest,
                modulePolicySha256
              });
            await enterStage('artifact_lookup');
            const cachedByDigest = await artifactCache.get(digestIdentity);
            throwIfAborted(signal);
            if (cachedByDigest.status === 'hit') {
              identity = digestIdentity;
              artifact = cachedByDigest.artifact;
              artifactCacheTier = cachedByDigest.tier ?? 'l2';
              // Drain any inline startup prefetched entry into `sourceCache` in background
              // so JSX Studio stays warm without retaining unconsumed startup entries.
              const prefetched = readStartupPrefetchedBlockNodeCode({
                actorId: scopedActorId,
                workspaceId: request.workspaceId,
                pageId: request.pageId,
                blockId: request.blockId
              });
              if (prefetched) {
                consumeStartupPrefetchedBlockNodeCode({
                  actorId: scopedActorId,
                  workspaceId: request.workspaceId,
                  pageId: request.pageId,
                  blockId: request.blockId,
                  promise: prefetched
                });
                void prefetched
                  .then((code) => {
                    sourceCopies.set(request.requestId, code);
                    return resolvedSourceCache?.put(
                      {
                        actorId: scopedActorId,
                        workspaceId: request.workspaceId,
                        requestId: request.requestId
                      },
                      code
                    );
                  })
                  .catch(() => undefined);
              }
            }
          }

          if (!artifact || !identity) {
            source = prewarmed
              ? await consumePrewarmSourceFlight(prewarmed, signal)
              : await readValidatedBlockSource({
                  actorId: scopedActorId,
                  request,
                  signal,
                  forceCompile,
                  fetchSource,
                  sourceCopies,
                  sourceCache: resolvedSourceCache
                });
            throwIfAborted(signal);
            // A conditional 304 read returns the same validated browser copy.
            // ACL was checked by the backend; the mounted factory needs no work.
            if (!forceCompile && source === preparedSource && preparedCopy)
              return preparedCopy;
            const legacyDiagnostic = diagnoseLegacyBlockModuleSource(
              source.source_code
            );
            if (legacyDiagnostic) {
              throw new NativeReactSourceContractError(legacyDiagnostic);
            }
            identity = createFrontstageNativeReactArtifactCacheIdentity({
              actorId: scopedActorId,
              workspaceId: readPlan.workspaceId,
              source: source.source_code,
              modulePolicySha256
            });
            if (!forceCompile && !knownDigest) {
              await enterStage('artifact_lookup');
              const cached = await artifactCache.get(identity);
              throwIfAborted(signal);
              if (cached.status === 'hit') {
                artifact = cached.artifact;
                artifactCacheTier = cached.tier ?? 'l2';
              }
            }
          }
          if (!artifact) {
            if (!source) {
              throw new Error('Block source is required on artifact cache miss.');
            }
            prewarmNativeModuleSources(source.source_code);
            await enterStage('compile', 'miss');
            const compileFlightKey = forceCompile
              ? undefined
              : createFrontstageNativeReactArtifactCacheKey(identity);
            const currentPriority = forceCompile
              ? 0
              : resolveFrontstageRuntimeDemand(
                  demandsRef.current,
                  request.blockId,
                  request.slotIndex
                );
            if (compileFlightKey) {
              activeCompileFlightKeys.set(request.blockId, compileFlightKey);
            }
            let compiled;
            try {
              compiled = await compile({
                source: source.source_code,
                requestId: `${request.requestId}:${identity.source_sha256}:refresh:${refreshGeneration}`,
                moduleDefinitions,
                signal,
                priority: currentPriority,
                slotIndex: request.slotIndex,
                ...(compileFlightKey ? { flightKey: compileFlightKey } : {})
              });
            } finally {
              if (
                compileFlightKey &&
                activeCompileFlightKeys.get(request.blockId) ===
                  compileFlightKey
              ) {
                activeCompileFlightKeys.delete(request.blockId);
              }
            }
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
            const moduleRegistry = createEvaluationRegistry();
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
                const moduleAssets = moduleRegistry.resolveInjectedModuleAssets
                  ? await moduleRegistry.resolveInjectedModuleAssets(
                      evaluated.artifact.program.injectedModules
                    )
                  : await moduleRegistry.resolveModuleAssets(moduleSources);
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
          preparedSource = source ?? undefined;
          return preparedCopy;
        }
      };
    });
  }, [
    scopedActorId,
    scopedWorkspaceId,
    activeCompileFlightKeys,
    artifactCache,
    compile,
    catalogEntries,
    componentFactoryFlights,
    fetchSource,
    moduleDefinitions,
    createEvaluationRegistry,
    modulePolicySha256,
    readPlan,
    refreshGenerationsByRequestId,
    resolvedSourceCache,
    sourceCopies,
    prewarmFlights,
    compiledRefreshes
  ]);

  useLayoutEffect(() => {
    if (
      !active ||
      !scopedActorId ||
      !readPlan ||
      scopedWorkspaceId !== readPlan.workspaceId
    ) {
      abortPrewarmFlights(prewarmFlights);
      return;
    }
    if (catalogEntries !== null) return;

    const candidates = createFrontstageRuntimeDemandCandidates(
      readPlan.requests,
      demandsByBlockId
    );
    const activeRequestIds = new Set(
      candidates.map(({ value }) => value.requestId)
    );
    for (const [requestId, flight] of prewarmFlights) {
      if (!activeRequestIds.has(requestId)) {
        flight.controller.abort();
        prewarmFlights.delete(requestId);
      }
    }
    for (const { value: request } of candidates.slice(0, maxConcurrent)) {
      if (prewarmFlights.has(request.requestId)) continue;
      // When `source_sha256` is already known and no inline startup code is buffered in RAM,
      // let `prepare()` check IndexedDB `artifactCache` first instead of firing speculative
      // `/blocks/:id/code` HTTP requests on warm reloads.
      if (
        fetchSource === defaultFetchSource &&
        typeof request.source_sha256 === 'string' &&
        /^[0-9a-f]{64}$/iu.test(request.source_sha256.trim()) &&
        !readStartupPrefetchedBlockNodeCode({
          actorId: scopedActorId,
          workspaceId: request.workspaceId,
          pageId: request.pageId,
          blockId: request.blockId
        })
      ) {
        continue;
      }
      const controller = new AbortController();
      const promise = readValidatedBlockSource({
        actorId: scopedActorId,
        request,
        signal: controller.signal,
        forceCompile: false,
        fetchSource,
        sourceCopies,
        sourceCache: resolvedSourceCache
      });
      const entry: PrewarmSourceFlight = { controller, promise };
      prewarmFlights.set(request.requestId, entry);
      void promise.catch(() => {
        if (prewarmFlights.get(request.requestId) === entry) {
          prewarmFlights.delete(request.requestId);
        }
      });
    }
  }, [
    active,
    catalogEntries,
    demandsByBlockId,
    fetchSource,
    maxConcurrent,
    prewarmFlights,
    readPlan,
    resolvedSourceCache,
    scopedActorId,
    scopedWorkspaceId,
    sourceCopies
  ]);

  useLayoutEffect(() => {
    if (!active) scheduler.suspend();
    if (demandsByBlockId) {
      for (const [blockId, priority] of Object.entries(demandsByBlockId)) {
        const flightKey = activeCompileFlightKeys.get(blockId);
        if (flightKey && priority <= 1) {
          promoteDefaultNativeReactCompilerFlight(flightKey, priority);
        }
      }
    }
    scheduler.reconcile(tasks, demandsByBlockId);
  }, [active, activeCompileFlightKeys, demandsByBlockId, scheduler, tasks]);

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

  useEffect(
    () => () => {
      abortPrewarmFlights(prewarmFlights);
      scheduler.dispose();
    },
    [prewarmFlights, scheduler]
  );

  useEffect(
    () =>
      subscribeFrontstageSourceChanges((change) => {
        if (
          change.workspaceId !== readPlan?.workspaceId ||
          change.pageId !== readPlan.pageId
        )
          return;
        invalidateStartupPrefetchedBlockNodeCode({
          actorId: scopedActorId,
          workspaceId: change.workspaceId,
          pageId: change.pageId,
          blockId: change.blockId
        });
        const request = readPlan.requests.find(
          (entry) => entry.blockId === change.blockId
        );
        if (request) {
          const flight = prewarmFlights.get(request.requestId);
          if (flight) {
            flight.controller.abort();
            prewarmFlights.delete(request.requestId);
          }
          sourceCopies.delete(request.requestId);
          if (scopedActorId) {
            void resolvedSourceCache
              ?.delete({
                actorId: scopedActorId,
                workspaceId: request.workspaceId,
                requestId: request.requestId
              })
              .catch(() => undefined);
          }
        }
        scheduler.retry(change.blockId);
      }),
    [
      prewarmFlights,
      readPlan,
      resolvedSourceCache,
      scheduler,
      scopedActorId,
      sourceCopies
    ]
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
      if (!request || !readPlan) return;
      invalidateStartupPrefetchedBlockNodeCode({
        actorId: scopedActorId,
        workspaceId: readPlan.workspaceId,
        pageId: readPlan.pageId,
        blockId
      });
      setRefreshGenerationsByRequestId((current) => ({
        ...current,
        [request.requestId]: (current[request.requestId] ?? 0) + 1
      }));
    },
    [readPlan, scopedActorId]
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
  cached?: NativePreparationSource,
  actorId?: string | null,
  forceCompile = false
): Promise<NativePreparationSource> {
  throwIfAborted(signal);
  if (!forceCompile && actorId) {
    const startupFlight = readStartupPrefetchedBlockNodeCode({
      actorId,
      workspaceId: request.workspaceId,
      pageId: request.pageId,
      blockId: request.blockId
    });
    if (startupFlight) {
      const prefetched = await startupFlight.catch(() => null);
      throwIfAborted(signal);
      consumeStartupPrefetchedBlockNodeCode({
        actorId,
        workspaceId: request.workspaceId,
        pageId: request.pageId,
        blockId: request.blockId,
        promise: startupFlight
      });
      if (prefetched) {
        if (
          cached?.source_sha256 &&
          prefetched.source_sha256 === cached.source_sha256
        ) {
          return cached;
        }
        return prefetched;
      }
    }
  }
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

function abortPrewarmFlights(
  prewarmFlights: Map<string, PrewarmSourceFlight>
): void {
  for (const flight of prewarmFlights.values()) {
    flight.controller.abort();
  }
  prewarmFlights.clear();
}

function consumePrewarmSourceFlight(
  flight: PrewarmSourceFlight,
  signal: AbortSignal
): Promise<NativePreparationSource> {
  if (signal.aborted) {
    flight.controller.abort();
    return Promise.reject(
      signal.reason ?? new DOMException('Preparation aborted.', 'AbortError')
    );
  }
  return new Promise<NativePreparationSource>((resolve, reject) => {
    const onAbort = () => {
      flight.controller.abort();
      reject(
        signal.reason ?? new DOMException('Preparation aborted.', 'AbortError')
      );
    };
    signal.addEventListener('abort', onAbort, { once: true });
    flight.promise.then(
      (value) => {
        signal.removeEventListener('abort', onAbort);
        resolve(value);
      },
      (error: unknown) => {
        signal.removeEventListener('abort', onAbort);
        reject(error);
      }
    );
  });
}

async function readValidatedBlockSource({
  actorId,
  request,
  signal,
  forceCompile,
  fetchSource,
  sourceCopies,
  sourceCache
}: {
  actorId: string;
  request: FrontstagePageCanvasBlockCodeReadPlan['requests'][number];
  signal: AbortSignal;
  forceCompile: boolean;
  fetchSource: NonNullable<
    UseFrontstagePageCanvasNativePreparationsInput['fetchSource']
  >;
  sourceCopies: Map<string, NativePreparationSource>;
  sourceCache: NativePreparationSourceCache | null;
}): Promise<NativePreparationSource> {
  const sourceWriteEpoch = sourceCache?.captureWriteEpoch?.(actorId);
  let cachedSource = forceCompile
    ? undefined
    : sourceCopies.get(request.requestId);
  if (!forceCompile && !cachedSource && sourceCache) {
    cachedSource = await sourceCache
      .get({
        actorId,
        workspaceId: request.workspaceId,
        requestId: request.requestId
      })
      .catch(() => undefined);
  }
  throwIfAborted(signal);
  let source: NativePreparationSource;
  try {
    source = await (fetchSource === defaultFetchSource
      ? defaultFetchSource(request, signal, cachedSource, actorId, forceCompile)
      : fetchSource(request, signal, cachedSource));
  } catch (error) {
    throw new FrontstageNativeSourceReadError(error);
  }
  throwIfAborted(signal);
  // The backend digest is the conditional-read contract. Reject corrupt
  // source before it can become a trusted browser copy or artifact.
  if (
    source.source_sha256 &&
    sha256Text(source.source_code) !== source.source_sha256.toLowerCase()
  ) {
    sourceCopies.delete(request.requestId);
    void sourceCache
      ?.delete({
        actorId,
        workspaceId: request.workspaceId,
        requestId: request.requestId
      })
      .catch(() => undefined);
    throw new Error('Native React source revision does not match its content.');
  }
  sourceCopies.set(request.requestId, source);
  if (source.source_sha256 && sourceCache) {
    void sourceCache
      .put(
        {
          actorId,
          workspaceId: request.workspaceId,
          requestId: request.requestId
        },
        source,
        sourceWriteEpoch
      )
      .catch(() => undefined);
  }
  return source;
}

function throwIfAborted(signal: AbortSignal): void {
  if (signal.aborted)
    throw new DOMException('Preparation aborted.', 'AbortError');
}
