import { act, renderHook, waitFor } from '@testing-library/react';
import type {
  JsBlockHostInterfaceEffect,
  JsBlockHostEffectHandler
} from '@1flowbase/page-runtime';
import { beforeEach, describe, expect, test, vi } from 'vitest';

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
      source: 'export default { render() {} }',
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
      handlers: { interface: interfaceEffectHandler }
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

  test('reports factory errors as stable entries instead of crashing', async () => {
    const failure = new Error('factory failed');
    const runtimeRunPlanState = createRunPlanState([createReadyItem()]);
    const runtimeSessionFactory: FrontstagePageCanvasRuntimeSessionFactory =
      vi.fn(() => {
        throw failure;
      });

    const { result } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
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
    const firstRender = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
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
    firstRender.unmount();

    const baseline = Date.now();
    const dateNow = vi
      .spyOn(Date, 'now')
      .mockReturnValue(baseline + 30_001);
    const revalidation = createFakeRuntimeSession();
    const revalidationFactory = vi.fn(() => revalidation.session);
    const restoredEffectHandler: JsBlockHostEffectHandler<JsBlockHostInterfaceEffect> =
      vi.fn(async () => ({ ok: true }));
    const secondRender = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        runtimeRunPlanState,
        runtimeSessionFactory: revalidationFactory,
        handlers: { interface: restoredEffectHandler }
      })
    );

    try {
      await waitFor(() => {
        expect(secondRender.result.current.entries[0]).toMatchObject({
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
      const restoredSnapshot = secondRender.result.current.entries[0];
      expect(revalidationFactory).not.toHaveBeenCalled();
      expect(revalidation.session.run).not.toHaveBeenCalled();
      expect(restoredEffectHandler).not.toHaveBeenCalled();
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
    const firstRender = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
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
    firstRender.unmount();

    const changedRequestIdItem = createReadyItem({
      blockId: 'request-stable',
      runPlan: createRunPlan({
        blockId: 'request-stable',
        requestId: 'request-id-after-remount',
        source: 'raw source is not the authoritative identity'
      })
    });
    const unexpectedFactory = vi.fn(() => createFakeRuntimeSession().session);
    const secondRender = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        runtimeRunPlanState: createRunPlanState([changedRequestIdItem]),
        runtimeSessionFactory: unexpectedFactory
      })
    );

    await waitFor(() => {
      expect(secondRender.result.current.entries[0]).toMatchObject({
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
    const firstRender = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
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
    firstRender.unmount();

    const retrySession = createFakeRuntimeSession();
    const runtimeSessionFactory = vi.fn(() => retrySession.session);
    const { result } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
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
