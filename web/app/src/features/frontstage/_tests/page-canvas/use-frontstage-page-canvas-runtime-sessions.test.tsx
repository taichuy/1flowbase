import { act, renderHook, waitFor } from '@testing-library/react';
import type {
  CompiledBlockArtifact,
  JsBlockHostInterfaceEffect,
  JsBlockHostEffectHandler
} from '@1flowbase/page-runtime';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import { IDBFactory } from 'fake-indexeddb';

import type { FrontstageRestrictedBlockRuntimeSession } from '../../lib/frontstage-restricted-block-runtime-host';
import type {
  FrontstagePageCanvasRuntimeRunPlanItem,
  FrontstagePageCanvasRuntimeRunPlanReadyItem,
  FrontstagePageCanvasRuntimeRunPlanState
} from '../../lib/page-canvas/runtime-run-plan';
import type { RestrictedBlockRunPlan } from '../../lib/restricted-block-loader';
import type { RestrictedBlockRuntimeHostSnapshot } from '../../lib/restricted-block-runtime-host';
import {
  clearFrontstageRuntimeSessionCache,
  FrontstageRuntimeResultCache,
  useFrontstagePageCanvasRuntimeSessions,
  type FrontstagePageCanvasRuntimeSessionFactory
} from '../../hooks/use-frontstage-page-canvas-runtime-sessions';
import {
  readFrontstageRuntimeObservations,
  recordFrontstageRuntimeObservation,
  resetFrontstageRuntimeObservations
} from '../../lib/page-canvas/runtime-observation';
import {
  FrontstageCompiledArtifactCache,
  createIndexedDbArtifactCacheStore
} from '../../lib/runtime-cache';

const TEST_RUNTIME_ACTOR = {
  actorId: 'actor-1',
  actorWorkspaceId: 'workspace-1'
} as const;

function compiledArtifact(
  sourceSha256 = 'a'.repeat(64)
): CompiledBlockArtifact {
  return {
    format: '1flowbase/js-block-compiled-artifact',
    version: 1,
    runtimeFingerprint: 'runtime-a',
    sourceSha256,
    program: {
      injectedModules: [],
      importBindings: [],
      executableBody: 'return { main: async () => ({ view: {}, outputs: {} }) };',
      executablePreambleLines: 0,
      moduleMapIdentifier: '__modules',
      defaultExportIdentifier: '__default'
    },
    manifest: { allowedImports: [] }
  };
}

function createRunPlan(
  overrides: Partial<RestrictedBlockRunPlan['request']> = {}
): RestrictedBlockRunPlan {
  const blockId = overrides.blockId ?? 'hero';
  const requestId =
    overrides.requestId ?? `restricted-block:${blockId}:hero-code`;

  return {
    ok: true,
    request: {
      requestId,
      blockId,
      program: {
        kind: 'source',
        source: 'export default { render() {} }'
      },
      props: { title: 'Hello' },
      state: {},
      contextSnapshot: { pageId: 'page-1' },
      limits: {
        timeoutMs: 1000,
        maxRenderDepth: 8,
        maxRenderNodes: 250
      },
      ...overrides
    },
    schemaValidationOptions: {
      maxDepth: 8,
      maxNodes: 250,
      allowedDataPermissions: ['query'],

      allowedEvents: ['record.saved']
    },
    mediatorPolicy: {
      allowedEvents: ['record.saved'],
      maxEventChainDepth: 4
    }
  };
}

function createSnapshot(
  overrides: Partial<RestrictedBlockRuntimeHostSnapshot> = {}
): RestrictedBlockRuntimeHostSnapshot {
  return {
    status: 'idle',
    requestId: 'restricted-block:hero:hero-code',
    blockId: 'hero',
    schemaValidationOptions: {
      maxDepth: 8,
      maxNodes: 250,
      allowedDataPermissions: ['query'],

      allowedEvents: ['record.saved']
    },
    logs: [],
    effects: [],
    rejections: [],
    ...overrides
  };
}

function createReadyItem({
  blockId = 'hero',
  codeRef = 'hero-code',
  source_sha256 = `${blockId}-source-sha256`,
  slotIndex = 0,
  sourceIndex = slotIndex,
  runPlan = createRunPlan({
    blockId,
    requestId: `restricted-block:${blockId}:${codeRef}`
  })
}: {
  blockId?: string;
  codeRef?: string;
  source_sha256?: string;
  slotIndex?: number;
  sourceIndex?: number;
  runPlan?: RestrictedBlockRunPlan;
} = {}): FrontstagePageCanvasRuntimeRunPlanReadyItem {
  return {
    status: 'run_plan_ready',
    blockId,
    sourceBlockId: blockId,
    codeRef,
    sourceCodeRef: codeRef,
    order: slotIndex,
    sourceIndex,
    slotIndex,
    renderMode: 'restricted_js_block',
    canEnterRestrictedJsRuntime: true,
    runtimeKind: 'iframe',
    runtimeEntry: `blocks/${blockId}.js`,
    contributionCode: `official.${blockId}`,
    sourceStatus: 'ready',
    source_sha256,
    catalogId: `official:${blockId}`,
    runPlan
  };
}

function createSkippedItem(
  status: Exclude<
    FrontstagePageCanvasRuntimeRunPlanItem['status'],
    'run_plan_ready'
  >,
  slotIndex: number
): FrontstagePageCanvasRuntimeRunPlanItem {
  const base = {
    blockId: `${status}-block`,
    sourceBlockId: `${status}-block`,
    codeRef: `${status}-code`,
    sourceCodeRef: `${status}-code`,
    order: slotIndex,
    sourceIndex: slotIndex,
    slotIndex,
    renderMode: 'restricted_js_block' as const,
    canEnterRestrictedJsRuntime: true,
    runtimeKind: 'iframe',
    runtimeEntry: `blocks/${status}.js`,
    contributionCode: `official.${status}`
  };

  if (status === 'source_not_ready') {
    return {
      ...base,
      status,
      sourceStatus: 'loading',
      reason: {
        code: 'source_not_ready',
        path: `sources.${slotIndex}.status`,
        message: 'waiting for source'
      }
    };
  }

  if (status === 'artifact_lookup_pending') {
    return {
      ...base,
      status,
      sourceStatus: 'ready',
      reason: {
        code: 'artifact_lookup_pending',
        path: `sources.${slotIndex}.artifactLookupStatus`,
        message: 'waiting for artifact lookup'
      }
    };
  }

  if (status === 'catalog_missing') {
    return {
      ...base,
      status,
      sourceStatus: 'ready',
      reason: {
        code: 'catalog_missing',
        path: 'catalogEntries',
        message: 'missing catalog'
      }
    };
  }

  return {
    ...base,
    status,
    sourceStatus: 'ready',
    catalogId: `official:${status}`,
    rejection: {
      ok: false,
      code: 'missing_limits',
      path: 'limits',
      message: 'missing limits'
    }
  };
}

function createRunPlanState(
  items: FrontstagePageCanvasRuntimeRunPlanItem[]
): FrontstagePageCanvasRuntimeRunPlanState {
  return {
    workspaceId: 'workspace-1',
    pageId: 'page-1',
    items
  };
}

function createFakeRuntimeSession(
  initialSnapshot: RestrictedBlockRuntimeHostSnapshot = createSnapshot()
) {
  type SnapshotListener = Parameters<
    FrontstageRestrictedBlockRuntimeSession['subscribe']
  >[0];
  type RuntimeSessionState = ReturnType<
    FrontstageRestrictedBlockRuntimeSession['getHostState']
  >;

  let snapshot = initialSnapshot;
  const listeners = new Set<SnapshotListener>();
  const callOrder: string[] = [];
  const unsubscribe = vi.fn((listener: SnapshotListener) => {
    listeners.delete(listener);
  });
  const session: FrontstageRestrictedBlockRuntimeSession = {
    run: vi.fn(() => {
      callOrder.push('run');
      snapshot = createSnapshot({
        requestId: snapshot.requestId,
        blockId: snapshot.blockId,
        status: 'running'
      });
      return snapshot;
    }),
    dispose: vi.fn(() => {
      callOrder.push('dispose');
      snapshot = createSnapshot({
        requestId: snapshot.requestId,
        blockId: snapshot.blockId,
        status: 'disposed'
      });
      return snapshot;
    }),
    getSnapshot: vi.fn(() => snapshot),
    getHostState: vi.fn(
      () =>
        ({
          workerStatus: 'idle',
          requests: {},
          rejections: []
        }) satisfies RuntimeSessionState
    ),
    subscribe: vi.fn((listener: SnapshotListener) => {
      callOrder.push('subscribe');
      listeners.add(listener);
      return () => unsubscribe(listener);
    })
  };

  return {
    session,
    callOrder,
    unsubscribe,
    emit(nextSnapshot: RestrictedBlockRuntimeHostSnapshot) {
      snapshot = nextSnapshot;
      for (const listener of [...listeners]) {
        listener(snapshot);
      }
    }
  };
}

describe('useFrontstagePageCanvasRuntimeSessions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearFrontstageRuntimeSessionCache();
    resetFrontstageRuntimeObservations();
    Object.defineProperty(document, 'visibilityState', {
      configurable: true,
      value: 'visible'
    });
  });

  test('creates, subscribes, and runs ready sessions with snapshots aligned by slot', async () => {
    const readyItem = createReadyItem({ blockId: 'hero', slotIndex: 2 });
    const runtimeSession = createFakeRuntimeSession(
      createSnapshot({
        requestId: readyItem.runPlan.request.requestId,
        blockId: readyItem.blockId
      })
    );
    const runtimeSessionFactory = vi.fn(() => runtimeSession.session);
    const interfaceEffectHandler: JsBlockHostEffectHandler<JsBlockHostInterfaceEffect> =
      vi.fn(async () => ({ ok: true }));
    const runtimeRunPlanState = createRunPlanState([readyItem]);

    const { result } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState,
        runtimeSessionFactory,
        handlers: { interface: interfaceEffectHandler }
      })
    );

    await waitFor(() => {
      expect(runtimeSessionFactory).toHaveBeenCalledTimes(1);
      expect(result.current.entries[0]).toMatchObject({
        status: 'running',
        blockId: 'hero',
        codeRef: 'hero-code',
        slotIndex: 2,
        snapshot: {
          status: 'running',
          requestId: 'restricted-block:hero:hero-code',
          blockId: 'hero'
        }
      });
    });

    expect(runtimeSessionFactory).toHaveBeenCalledWith({
      runPlan: readyItem.runPlan,
      handlers: { interface: interfaceEffectHandler },
      runtimeFingerprint: expect.any(String)
    });
    expect(runtimeSession.callOrder).toEqual(['subscribe', 'run']);
    expect(result.current.snapshotsBySlot[2]).toMatchObject({
      status: 'running',
      blockId: 'hero'
    });
    expect(result.current.running).toBe(true);
    expect(result.current.hasError).toBe(false);
  });

  test('skips non-ready run plan items without creating sessions', async () => {
    const runtimeSessionFactory = vi.fn(
      () => createFakeRuntimeSession().session
    );
    const runtimeRunPlanState = createRunPlanState([
      createSkippedItem('source_not_ready', 0),
      createSkippedItem('catalog_missing', 1),
      createSkippedItem('rejected', 2)
    ]);

    const { result } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState,
        runtimeSessionFactory
      })
    );

    await waitFor(() => {
      expect(result.current.entries).toHaveLength(3);
    });

    expect(runtimeSessionFactory).not.toHaveBeenCalled();
    expect(result.current.entries).toEqual([
      expect.objectContaining({
        status: 'skipped',
        skipReason: 'source_not_ready',
        sourceStatus: 'loading',
        slotIndex: 0,
        message: 'waiting for source'
      }),
      expect.objectContaining({
        status: 'skipped',
        skipReason: 'catalog_missing',
        slotIndex: 1,
        message: 'missing catalog'
      }),
      expect.objectContaining({
        status: 'skipped',
        skipReason: 'rejected',
        slotIndex: 2,
        message: 'missing limits'
      })
    ]);
    expect(result.current.snapshotsBySlot).toEqual({});
    expect(result.current.running).toBe(false);
    expect(result.current.hasError).toBe(true);
  });

  test('does not run ready items for anonymous or mismatched actors', async () => {
    const runtimeSessionFactory = vi.fn(
      () => createFakeRuntimeSession().session
    );
    const runtimeRunPlanState = createRunPlanState([createReadyItem()]);
    const { result: anonymousResult } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        actorId: null,
        actorWorkspaceId: null,
        runtimeRunPlanState,
        runtimeSessionFactory
      })
    );
    const { result: mismatchedResult } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        actorId: 'actor-1',
        actorWorkspaceId: 'workspace-2',
        runtimeRunPlanState,
        runtimeSessionFactory
      })
    );

    await waitFor(() => {
      expect(anonymousResult.current.entries).toEqual([]);
      expect(mismatchedResult.current.entries).toEqual([]);
    });
    expect(runtimeSessionFactory).not.toHaveBeenCalled();
  });

  test('disposes sessions that no longer match after the run plan changes', async () => {
    const firstItem = createReadyItem({ blockId: 'hero', slotIndex: 0 });
    const secondItem = createReadyItem({ blockId: 'gallery', slotIndex: 0 });
    const firstRuntimeSession = createFakeRuntimeSession(
      createSnapshot({
        requestId: firstItem.runPlan.request.requestId,
        blockId: firstItem.blockId
      })
    );
    const secondRuntimeSession = createFakeRuntimeSession(
      createSnapshot({
        requestId: secondItem.runPlan.request.requestId,
        blockId: secondItem.blockId
      })
    );
    const sessions = [
      firstRuntimeSession.session,
      secondRuntimeSession.session
    ];
    const runtimeSessionFactory = vi.fn(() => sessions.shift()!);

    const { result, rerender } = renderHook(
      ({ runtimeRunPlanState }) =>
        useFrontstagePageCanvasRuntimeSessions({
          ...TEST_RUNTIME_ACTOR,
          runtimeRunPlanState,
          runtimeSessionFactory
        }),
      {
        initialProps: {
          runtimeRunPlanState: createRunPlanState([firstItem])
        }
      }
    );

    await waitFor(() => {
      expect(result.current.entries[0]).toMatchObject({
        status: 'running',
        blockId: 'hero'
      });
    });

    rerender({ runtimeRunPlanState: createRunPlanState([secondItem]) });

    await waitFor(() => {
      expect(result.current.entries[0]).toMatchObject({
        status: 'running',
        blockId: 'gallery'
      });
    });

    expect(firstRuntimeSession.unsubscribe).toHaveBeenCalledTimes(1);
    expect(firstRuntimeSession.session.dispose).toHaveBeenCalledTimes(1);
    expect(secondRuntimeSession.session.run).toHaveBeenCalledTimes(1);
  });

  test('disposes active sessions on unmount', async () => {
    const runtimeSession = createFakeRuntimeSession();
    const runtimeSessionFactory = vi.fn(() => runtimeSession.session);
    const runtimeRunPlanState = createRunPlanState([createReadyItem()]);

    const { unmount } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState,
        runtimeSessionFactory
      })
    );

    await waitFor(() => {
      expect(runtimeSession.session.run).toHaveBeenCalledTimes(1);
    });

    unmount();

    expect(runtimeSession.unsubscribe).toHaveBeenCalledTimes(1);
    expect(runtimeSession.session.dispose).toHaveBeenCalledTimes(1);
  });

  test('updates entries when a session emits ready and failed snapshots', async () => {
    const readyItem = createReadyItem({ blockId: 'hero', slotIndex: 1 });
    const runtimeSession = createFakeRuntimeSession(
      createSnapshot({
        requestId: readyItem.runPlan.request.requestId,
        blockId: readyItem.blockId
      })
    );
    const runtimeRunPlanState = createRunPlanState([readyItem]);
    const runtimeSessionFactory = vi.fn(() => runtimeSession.session);

    const { result } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState,
        runtimeSessionFactory
      })
    );

    await waitFor(() => {
      expect(result.current.entries[0]?.status).toBe('running');
    });

    act(() => {
      runtimeSession.emit(
        createSnapshot({
          requestId: readyItem.runPlan.request.requestId,
          blockId: readyItem.blockId,
          status: 'ready',
          view: {
            primitive: 'Text',
            props: { children: 'Runtime Ready' }
          }
        })
      );
    });

    expect(result.current.entries[0]).toMatchObject({
      status: 'ready',
      snapshot: {
        status: 'ready',
        view: {
          primitive: 'Text',
          props: { children: 'Runtime Ready' }
        }
      }
    });
    expect(result.current.running).toBe(false);
    expect(result.current.hasError).toBe(false);

    act(() => {
      runtimeSession.emit(
        createSnapshot({
          requestId: readyItem.runPlan.request.requestId,
          blockId: readyItem.blockId,
          status: 'failed',
          error: {
            kind: 'runtime_error',
            message: 'Worker failed.',
            errors: [
              {
                code: 'runtime_error',
                path: 'runtime',
                message: 'Worker failed.'
              }
            ]
          }
        })
      );
    });

    expect(result.current.entries[0]).toMatchObject({
      status: 'failed',
      snapshot: {
        status: 'failed',
        error: { message: 'Worker failed.' }
      }
    });
    expect(result.current.hasError).toBe(true);
  });

  test('records bounded runtime phase transitions without sensitive runtime values', async () => {
    const sensitiveCanary = 'sensitive-runtime-canary';
    recordFrontstageRuntimeObservation({
      stage: 'source_fetch',
      cacheTier: 'network',
      actorId: 'actor-1',
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      blockId: 'observed'
    });
    const item = createReadyItem({
      blockId: 'observed',
      runPlan: createRunPlan({
        blockId: 'observed',
        program: { kind: 'source', source: sensitiveCanary },
        props: { secret: sensitiveCanary },
        contextSnapshot: { token: sensitiveCanary }
      })
    });
    const runtimeSession = createFakeRuntimeSession();
    renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState: createRunPlanState([item]),
        runtimeSessionFactory: () => runtimeSession.session
      })
    );
    await waitFor(() =>
      expect(runtimeSession.session.run).toHaveBeenCalledTimes(1)
    );

    for (const phase of [
      'compiling',
      'waiting_effect',
      'executing',
      'validating_schema'
    ] as const) {
      act(() => {
        runtimeSession.emit(
          createSnapshot({
            status: 'running',
            phase,
            requestId: item.runPlan.request.requestId,
            blockId: item.blockId
          })
        );
      });
    }
    act(() => {
      runtimeSession.emit(
        createSnapshot({
          status: 'ready',
          phase: 'ready',
          requestId: item.runPlan.request.requestId,
          blockId: item.blockId,
          view: { primitive: 'Text', props: { children: sensitiveCanary } },
          outputs: { secret: sensitiveCanary }
        })
      );
    });

    const observations = readFrontstageRuntimeObservations();
    expect(observations.map((entry) => entry.stage)).toEqual([
      'source_fetch',
      'worker_boot',
      'compile',
      'api_wait',
      'main',
      'schema_validate',
      'present'
    ]);
    expect(observations.map((entry) => entry.count)).toEqual([
      1, 1, 1, 1, 1, 1, 1
    ]);
    expect(observations.map((entry) => entry.cacheTier)).toEqual([
      'network',
      'miss',
      'miss',
      'miss',
      'miss',
      'miss',
      'miss'
    ]);
    expect(JSON.stringify(observations)).not.toContain(sensitiveCanary);
  });

  test('AC-023 persists ready L2 artifacts without compile and ignores quota failure', async () => {
    recordFrontstageRuntimeObservation({
      stage: 'source_fetch',
      cacheTier: 'network',
      actorId: 'actor-1',
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      blockId: 'hero'
    });
    const sourceSha256 = 'a'.repeat(64);
    const artifact = compiledArtifact(sourceSha256);
    const item = createReadyItem({
      source_sha256: sourceSha256,
      runPlan: createRunPlan({
        program: {
          kind: 'compiled_artifact',
          artifact,
          sourceSha256,
          fallback: {
            kind: 'source',
            source: 'deliberately invalid fallback {'
          }
        }
      })
    });
    const runtimeSession = createFakeRuntimeSession();
    const artifactCache = {
      put: vi.fn(async () => ({
        status: 'unavailable' as const,
        reason: 'quota_exceeded' as const
      }))
    };
    const runtimeSessionFactory = vi.fn(() => runtimeSession.session);
    const { result } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState: createRunPlanState([item]),
        runtimeSessionFactory,
        artifactCache,
        runtimeFingerprint: 'runtime-a'
      })
    );
    await waitFor(() => expect(runtimeSession.session.run).toHaveBeenCalledTimes(1));
    for (const phase of ['executing', 'waiting_effect', 'validating_schema'] as const) {
      act(() =>
        runtimeSession.emit(
          createSnapshot({
            status: 'running',
            phase,
            requestId: item.runPlan.request.requestId,
            blockId: item.blockId
          })
        )
      );
    }
    act(() =>
      runtimeSession.emit(
        createSnapshot({
          status: 'ready',
          phase: 'ready',
          requestId: item.runPlan.request.requestId,
          blockId: item.blockId,
          outputs: { apiValue: 'fresh-response' },
          compiledArtifact: artifact
        })
      )
    );

    await waitFor(() =>
      expect(result.current.entries[0]).toMatchObject({ status: 'ready' })
    );
    expect(runtimeSessionFactory).toHaveBeenCalledWith(
      expect.objectContaining({ runtimeFingerprint: 'runtime-a' })
    );
    expect(artifactCache.put).toHaveBeenCalledWith(
      {
        actorId: 'actor-1',
        workspaceId: 'workspace-1',
        runtimeFingerprint: 'runtime-a',
        sourceSha256
      },
      artifact
    );
    expect(
      readFrontstageRuntimeObservations().map((entry) => [
        entry.stage,
        entry.cacheTier
      ])
    ).toEqual([
      ['source_fetch', 'network'],
      ['worker_boot', 'l2'],
      ['main', 'l2'],
      ['api_wait', 'l2'],
      ['schema_validate', 'l2'],
      ['present', 'l2']
    ]);
  });

  test('AC-022 stores no runtime canary after a real ready execution snapshot', async () => {
    const sourceSha256 = 'b'.repeat(64);
    const artifact = compiledArtifact(sourceSha256);
    const item = createReadyItem({ source_sha256: sourceSha256 });
    const runtimeSession = createFakeRuntimeSession();
    const store = createIndexedDbArtifactCacheStore({
      indexedDB: new IDBFactory(),
      databaseName: 'runtime-canary-scan'
    });
    const artifactCache = new FrontstageCompiledArtifactCache({ store });
    renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState: createRunPlanState([item]),
        runtimeSessionFactory: () => runtimeSession.session,
        artifactCache,
        runtimeFingerprint: 'runtime-a'
      })
    );
    await waitFor(() => expect(runtimeSession.session.run).toHaveBeenCalled());
    act(() =>
      runtimeSession.emit(
        createSnapshot({
          status: 'ready',
          phase: 'ready',
          requestId: item.runPlan.request.requestId,
          blockId: item.blockId,
          compiledArtifact: artifact,
          view: { primitive: 'Text', props: { children: 'runtime-secret-canary' } },
          outputs: { response: 'runtime-secret-canary' },
          logs: [{
            requestId: item.runPlan.request.requestId,
            level: 'info',
            message: 'runtime-secret-canary'
          }],
          effects: [{
            type: 'event',
            requestId: item.runPlan.request.requestId,
            name: 'runtime-secret-canary',
            payload: { token: 'runtime-secret-canary' }
          }],
          interfaceCalls: [{
            requestId: item.runPlan.request.requestId,
            effectId: 'runtime-secret-canary',
            method: 'GET',
            path: '/runtime-secret-canary',
            status: 'succeeded',
            durationMs: 1,
            response: { headers: 'runtime-secret-canary' }
          }]
        })
      )
    );
    await waitFor(async () => expect(await store.list()).toHaveLength(1));
    expect(JSON.stringify(await store.list())).not.toContain(
      'runtime-secret-canary'
    );
    await expect(
      artifactCache.get({
        actorId: 'actor-1',
        workspaceId: 'workspace-1',
        runtimeFingerprint: 'runtime-a',
        sourceSha256
      })
    ).resolves.toMatchObject({ status: 'hit', artifact });
  });

  test('reports factory errors as stable entries instead of crashing', async () => {
    const failure = new Error('factory failed');
    const runtimeRunPlanState = createRunPlanState([createReadyItem()]);
    const runtimeSessionFactory: FrontstagePageCanvasRuntimeSessionFactory =
      vi.fn(() => {
        throw failure;
      });

    const { result } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState,
        runtimeSessionFactory
      })
    );

    await waitFor(() => {
      expect(result.current.entries[0]).toMatchObject({
        status: 'factory_failed',
        blockId: 'hero',
        slotIndex: 0,
        message: 'factory failed',
        error: failure
      });
    });
    expect(result.current.running).toBe(false);
    expect(result.current.hasError).toBe(true);
  });

  test('runs at most two sessions and starts the highest-demand queued block next', async () => {
    const items = [
      createReadyItem({ blockId: 'far', slotIndex: 0 }),
      createReadyItem({ blockId: 'visible', slotIndex: 1 }),
      createReadyItem({ blockId: 'near', slotIndex: 2 })
    ];
    const sessions = items.map((item) =>
      createFakeRuntimeSession(
        createSnapshot({
          status: 'running',
          requestId: item.runPlan.request.requestId,
          blockId: item.blockId
        })
      )
    );
    const startedBlockIds: string[] = [];
    const runtimeSessionFactory: FrontstagePageCanvasRuntimeSessionFactory =
      vi.fn((options) => {
        startedBlockIds.push(options.runPlan.request.blockId);
        return sessions[startedBlockIds.length - 1]!.session;
      });
    const runtimeRunPlanState = createRunPlanState(items);

    renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState,
        runtimeSessionFactory,
        demandsByBlockId: { far: 3, visible: 1, near: 2 }
      })
    );

    await waitFor(() => expect(runtimeSessionFactory).toHaveBeenCalledTimes(2));
    expect(startedBlockIds).toEqual(['visible', 'near']);

    act(() => {
      sessions[0]!.emit(
        createSnapshot({
          status: 'ready',
          requestId: items[1]!.runPlan.request.requestId,
          blockId: 'visible',
          view: { primitive: 'Text', props: { children: 'ready' } }
        })
      );
    });

    await waitFor(() => expect(runtimeSessionFactory).toHaveBeenCalledTimes(3));
  });

  test('does not start queued work while the page is hidden and resumes when visible', async () => {
    Object.defineProperty(document, 'visibilityState', {
      configurable: true,
      value: 'hidden'
    });
    const runtimeSessionFactory = vi.fn(
      () => createFakeRuntimeSession().session
    );
    const runtimeRunPlanState = createRunPlanState([createReadyItem()]);

    renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState,
        runtimeSessionFactory
      })
    );
    expect(runtimeSessionFactory).not.toHaveBeenCalled();

    Object.defineProperty(document, 'visibilityState', {
      configurable: true,
      value: 'visible'
    });
    act(() => document.dispatchEvent(new Event('visibilitychange')));

    await waitFor(() => expect(runtimeSessionFactory).toHaveBeenCalledTimes(1));
  });

  test('restores a successful result beyond the former TTL without factory, run, or effects', async () => {
    const item = createReadyItem({ blockId: 'cached' });
    const runtimeRunPlanState = createRunPlanState([item]);
    const first = createFakeRuntimeSession(
      createSnapshot({
        status: 'running',
        requestId: item.runPlan.request.requestId,
        blockId: item.blockId
      })
    );
    const { unmount: unmountFirst } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState,
        runtimeSessionFactory: () => first.session
      })
    );
    await waitFor(() => expect(first.session.run).toHaveBeenCalledTimes(1));
    act(() => {
      first.emit(
        createSnapshot({
          status: 'ready',
          requestId: item.runPlan.request.requestId,
          blockId: item.blockId,
          view: { primitive: 'Text', props: { children: 'cached' } },
          outputs: { value: 'cached-output' },
          logs: [{ message: 'must not be cached' } as never],
          effects: [{ kind: 'must-not-be-cached' } as never],
          rejections: [{ code: 'must-not-be-cached' } as never],
          mediatorState: {} as never,
          interfaceCalls: [{ path: '/must-not-be-cached' } as never]
        })
      );
    });
    unmountFirst();
    resetFrontstageRuntimeObservations();

    const baseline = Date.now();
    const dateNow = vi
      .spyOn(Date, 'now')
      .mockReturnValue(baseline + 30_001);
    const revalidation = createFakeRuntimeSession();
    const revalidationFactory = vi.fn(() => revalidation.session);
    const persistentArtifactCache = {
      put: vi.fn(async () => ({ status: 'stored' as const, byteSize: 1 }))
    };
    const restoredEffectHandler: JsBlockHostEffectHandler<JsBlockHostInterfaceEffect> =
      vi.fn(async () => ({ ok: true }));
    const { result: secondResult } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState,
        runtimeSessionFactory: revalidationFactory,
        artifactCache: persistentArtifactCache,
        handlers: { interface: restoredEffectHandler }
      })
    );

    try {
      await waitFor(() => {
        expect(secondResult.current.entries[0]).toMatchObject({
          status: 'ready',
          snapshot: {
            view: { primitive: 'Text', props: { children: 'cached' } },
            outputs: { value: 'cached-output' },
            logs: [],
            effects: [],
            rejections: []
          }
        });
      });
      const restoredSnapshot = secondResult.current.entries[0];
      expect(revalidationFactory).not.toHaveBeenCalled();
      expect(revalidation.session.run).not.toHaveBeenCalled();
      expect(restoredEffectHandler).not.toHaveBeenCalled();
      expect(persistentArtifactCache.put).not.toHaveBeenCalled();
      expect(
        readFrontstageRuntimeObservations().map((entry) => [
          entry.stage,
          entry.cacheTier
        ])
      ).toEqual([['present', 'l1']]);
      expect(
        restoredSnapshot && 'snapshot' in restoredSnapshot
          ? restoredSnapshot.snapshot.mediatorState
          : null
      ).toBeUndefined();
      expect(
        restoredSnapshot && 'snapshot' in restoredSnapshot
          ? restoredSnapshot.snapshot.interfaceCalls
          : null
      ).toBeUndefined();
    } finally {
      dateNow.mockRestore();
    }
  });

  test('excludes requestId and raw source from identity while using the current requestId', async () => {
    const firstItem = createReadyItem({ blockId: 'request-stable' });
    const first = createFakeRuntimeSession();
    const { unmount: unmountFirst } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState: createRunPlanState([firstItem]),
        runtimeSessionFactory: () => first.session
      })
    );
    await waitFor(() => expect(first.session.run).toHaveBeenCalledTimes(1));
    act(() => {
      first.emit(
        createSnapshot({
          status: 'ready',
          requestId: firstItem.runPlan.request.requestId,
          blockId: firstItem.blockId,
          view: { primitive: 'Text', props: { children: 'stable' } }
        })
      );
    });
    unmountFirst();

    const changedRequestIdItem = createReadyItem({
      blockId: 'request-stable',
      runPlan: createRunPlan({
        blockId: 'request-stable',
        requestId: 'request-id-after-remount',
        program: {
          kind: 'source',
          source: 'raw source is not the authoritative identity'
        }
      })
    });
    const unexpectedFactory = vi.fn(() => createFakeRuntimeSession().session);
    const { result: secondResult } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState: createRunPlanState([changedRequestIdItem]),
        runtimeSessionFactory: unexpectedFactory
      })
    );

    await waitFor(() => {
      expect(secondResult.current.entries[0]).toMatchObject({
        status: 'ready',
        snapshot: { requestId: 'request-id-after-remount' }
      });
    });
    expect(unexpectedFactory).not.toHaveBeenCalled();
  });

  test('reruns only the affected block once for hash and explicit dependency changes', async () => {
    const hero = createReadyItem({ blockId: 'hero' });
    const stable = createReadyItem({ blockId: 'stable', slotIndex: 1 });
    const startedBlockIds: string[] = [];
    const runtimeSessionFactory: FrontstagePageCanvasRuntimeSessionFactory =
      vi.fn((options) => {
        startedBlockIds.push(options.runPlan.request.blockId);
        const session = createFakeRuntimeSession();
        return session.session;
      });
    const { rerender } = renderHook(
      ({ items }) =>
        useFrontstagePageCanvasRuntimeSessions({
          ...TEST_RUNTIME_ACTOR,
          runtimeRunPlanState: createRunPlanState(items),
          runtimeSessionFactory
        }),
      { initialProps: { items: [hero, stable] } }
    );
    await waitFor(() => expect(runtimeSessionFactory).toHaveBeenCalledTimes(2));

    const withHashChange = createReadyItem({
      blockId: 'hero',
      source_sha256: 'hero-source-sha256-v2'
    });
    rerender({ items: [withHashChange, stable] });
    await waitFor(() => expect(runtimeSessionFactory).toHaveBeenCalledTimes(3));

    const withPropsChange = createReadyItem({
      blockId: 'hero',
      source_sha256: 'hero-source-sha256-v2',
      runPlan: createRunPlan({
        blockId: 'hero',
        props: { title: 'Changed' }
      })
    });
    rerender({ items: [withPropsChange, stable] });
    await waitFor(() => expect(runtimeSessionFactory).toHaveBeenCalledTimes(4));

    const withContextChange = createReadyItem({
      blockId: 'hero',
      source_sha256: 'hero-source-sha256-v2',
      runPlan: createRunPlan({
        blockId: 'hero',
        props: { title: 'Changed' },
        contextSnapshot: { pageId: 'page-2' }
      })
    });
    rerender({ items: [withContextChange, stable] });
    await waitFor(() => expect(runtimeSessionFactory).toHaveBeenCalledTimes(5));

    const withInputsChange = createReadyItem({
      blockId: 'hero',
      source_sha256: 'hero-source-sha256-v2',
      runPlan: createRunPlan({
        blockId: 'hero',
        props: { title: 'Changed' },
        contextSnapshot: { pageId: 'page-2' },
        inputs: { selectedId: 'record-2' }
      })
    });
    rerender({ items: [withInputsChange, stable] });
    await waitFor(() => expect(runtimeSessionFactory).toHaveBeenCalledTimes(6));

    expect(startedBlockIds).toEqual([
      'hero',
      'stable',
      'hero',
      'hero',
      'hero',
      'hero'
    ]);
  });

  test('manual retry evicts the block result and runs that block exactly once', async () => {
    const item = createReadyItem({ blockId: 'retry' });
    const first = createFakeRuntimeSession();
    const { unmount: unmountFirst } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState: createRunPlanState([item]),
        runtimeSessionFactory: () => first.session
      })
    );
    await waitFor(() => expect(first.session.run).toHaveBeenCalledTimes(1));
    act(() => {
      first.emit(
        createSnapshot({
          status: 'ready',
          requestId: item.runPlan.request.requestId,
          blockId: item.blockId,
          view: { primitive: 'Text', props: { children: 'retry-cache' } }
        })
      );
    });
    unmountFirst();

    const retrySession = createFakeRuntimeSession();
    const runtimeSessionFactory = vi.fn(() => retrySession.session);
    const { result } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        ...TEST_RUNTIME_ACTOR,
        runtimeRunPlanState: createRunPlanState([item]),
        runtimeSessionFactory
      })
    );
    await waitFor(() => expect(result.current.entries[0]?.status).toBe('ready'));
    expect(runtimeSessionFactory).not.toHaveBeenCalled();

    act(() => result.current.retryBlock('retry'));

    await waitFor(() => expect(runtimeSessionFactory).toHaveBeenCalledTimes(1));
    expect(retrySession.session.run).toHaveBeenCalledTimes(1);
  });

  test('evicts deterministically by byte-weighted LRU under a hard budget', () => {
    const schemaValidationOptions = createSnapshot().schemaValidationOptions;
    const value = (label: string) => ({
      view: { primitive: 'Text', props: { children: label.repeat(64) } },
      outputs: { label },
      schemaValidationOptions
    });

    const probe = new FrontstageRuntimeResultCache(100_000);
    probe.set('a', value('a'));
    probe.set('b', value('b'));
    const cache = new FrontstageRuntimeResultCache(probe.byteSize);
    cache.set('a', value('a'));
    cache.set('b', value('b'));
    expect(cache.get('a')).toBeDefined();

    cache.set('c', value('c'));

    expect(cache.get('b')).toBeUndefined();
    expect(cache.get('a')).toBeDefined();
    expect(cache.get('c')).toBeDefined();
    expect(cache.byteSize).toBeLessThanOrEqual(cache.byteBudget);
  });
});
