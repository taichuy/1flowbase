import { act, renderHook, waitFor } from '@testing-library/react';
import type {
  JsBlockHostDataEffect,
  JsBlockHostEffectHandler
} from '@1flowbase/page-runtime';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import type {
  FrontstageRestrictedBlockRuntimeSession
} from '../../lib/frontstage-restricted-block-runtime-host';
import type {
  FrontstagePageCanvasRuntimeRunPlanItem,
  FrontstagePageCanvasRuntimeRunPlanReadyItem,
  FrontstagePageCanvasRuntimeRunPlanState
} from '../../lib/page-canvas/runtime-run-plan';
import type { RestrictedBlockRunPlan } from '../../lib/restricted-block-loader';
import type { RestrictedBlockRuntimeHostSnapshot } from '../../lib/restricted-block-runtime-host';
import {
  useFrontstagePageCanvasRuntimeSessions,
  type FrontstagePageCanvasRuntimeSessionFactory
} from '../../hooks/use-frontstage-page-canvas-runtime-sessions';

function createRunPlan(
  overrides: Partial<RestrictedBlockRunPlan['request']> = {}
): RestrictedBlockRunPlan {
  const blockId = overrides.blockId ?? 'hero';
  const requestId = overrides.requestId ?? `restricted-block:${blockId}:hero-code`;

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
      allowedActions: ['record.save'],
      allowedEvents: ['record.saved']
    },
    mediatorPolicy: {
      allowedQueries: ['records'],
      allowedDataOperations: ['query'],
      allowedActions: ['record.save'],
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
      allowedActions: ['record.save'],
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
  slotIndex = 0,
  sourceIndex = slotIndex,
  runPlan = createRunPlan({
    blockId,
    requestId: `restricted-block:${blockId}:${codeRef}`
  })
}: {
  blockId?: string;
  codeRef?: string;
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
    const dataEffectHandler: JsBlockHostEffectHandler<JsBlockHostDataEffect> =
      vi.fn(async () => ({ ok: true }));
    const runtimeRunPlanState = createRunPlanState([readyItem]);

    const { result } = renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        runtimeRunPlanState,
        runtimeSessionFactory,
        handlers: { data: dataEffectHandler }
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
      handlers: { data: dataEffectHandler }
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
    const runtimeSessionFactory = vi.fn(() => createFakeRuntimeSession().session);
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
    const sessions = [firstRuntimeSession.session, secondRuntimeSession.session];
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
          schema: {
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
        schema: {
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
    const runtimeSessionFactory: FrontstagePageCanvasRuntimeSessionFactory = vi.fn(
      () => {
        throw failure;
      }
    );

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
    const runtimeSessionFactory = vi.fn(() =>
      sessions[runtimeSessionFactory.mock.calls.length - 1]!.session
    );

    renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        runtimeRunPlanState: createRunPlanState(items),
        runtimeSessionFactory,
        demandsByBlockId: { far: 3, visible: 1, near: 2 }
      })
    );

    await waitFor(() => expect(runtimeSessionFactory).toHaveBeenCalledTimes(2));
    expect(runtimeSessionFactory.mock.calls.map(([options]) => options.runPlan.request.blockId))
      .toEqual(['visible', 'near']);

    act(() => {
      sessions[0]!.emit(
        createSnapshot({
          status: 'ready',
          requestId: items[1]!.runPlan.request.requestId,
          blockId: 'visible',
          schema: { primitive: 'Text', props: { children: 'ready' } }
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
    const runtimeSessionFactory = vi.fn(() => createFakeRuntimeSession().session);

    renderHook(() =>
      useFrontstagePageCanvasRuntimeSessions({
        runtimeRunPlanState: createRunPlanState([createReadyItem()]),
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
});
