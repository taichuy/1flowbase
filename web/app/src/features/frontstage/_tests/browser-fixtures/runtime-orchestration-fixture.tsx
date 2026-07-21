import '@ant-design/v5-patch-for-react-19';
import { ConfigProvider } from 'antd';
import { StrictMode, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { sha256Text } from '@1flowbase/page-runtime';

import { PageCanvas } from '../../components/PageCanvas';
import {
  createFrontstageRestrictedBlockRuntimeSession,
  type FrontstageRestrictedBlockRuntimeHostOptions,
  type FrontstageRestrictedBlockRuntimeSession
} from '../../lib/frontstage-restricted-block-runtime-host';
import type { FrontstageRuntimeDemandPriority } from '../../lib/page-canvas/runtime-demand';
import type {
  FrontstagePageCanvasRuntimeRunPlanReadyItem,
  FrontstagePageCanvasRuntimeRunPlanState
} from '../../lib/page-canvas/runtime-run-plan';
import { useFrontstagePageCanvasRuntimeSessions } from '../../hooks/use-frontstage-page-canvas-runtime-sessions';
import { createFrontstagePageContentFixture } from '../frontstage-page-content-fixtures';
import {
  frontstageCompiledArtifactCache,
  createIndexedDbArtifactCacheStore
} from '../../lib/runtime-cache';
import { clearFrontstageRuntimeSessionCache } from '../../hooks/use-frontstage-page-canvas-runtime-sessions';
import {
  readFrontstageRuntimeObservations,
  recordFrontstageRuntimeObservation,
  resetFrontstageRuntimeObservations
} from '../../lib/page-canvas/runtime-observation';
import { getFrontstageRestrictedBlockRuntimeFingerprint } from '../../lib/restricted-block-worker-factory';

const params = new URLSearchParams(window.location.search);
const blockCount = Math.min(
  50,
  Math.max(1, Number(params.get('blocks')) || 10)
);
const failingBlock = params.get('error') === '1';
const policyBlock = params.get('policy') === '1';
const nonSettlingBlock = params.get('infinite') === '1';

function createRunPlanItem(
  index: number
): FrontstagePageCanvasRuntimeRunPlanReadyItem {
  const blockId = `runtime-fixture-${index + 1}`;
  const codeRef = `${blockId}-code`;
  const source =
    policyBlock && index === 0
      ? `
import type { BlockModule, BlockResult } from '@1flowbase/block-sdk';

async function main(): Promise<BlockResult> {
  while (true) {}
}

export default { main } satisfies BlockModule;`
      : failingBlock && index === 0
        ? `
import type { BlockModule, BlockResult } from '@1flowbase/block-sdk';

async function main(): Promise<BlockResult> {
  throw new Error('Controlled fixture failure');
}

export default { main } satisfies BlockModule;`
        : `
import type {
  BlockContext,
  BlockModule,
  BlockResult
} from '@1flowbase/block-sdk';
import { Text } from '@1flowbase/block-renderer/antd-facade';

async function main(ctx: BlockContext): Promise<BlockResult> {
  const response = await ctx.api.get('/api/console/test', {
    body: { blockId: ctx.props.blockId }
  });

  return {
    view: Text({ children: 'Rendered ' + ctx.props.blockId }),
    outputs: { response }
  };
}

export default { main } satisfies BlockModule;`;

  return {
    status: 'run_plan_ready',
    blockId,
    sourceBlockId: blockId,
    codeRef,
    sourceCodeRef: codeRef,
    order: index,
    sourceIndex: index,
    slotIndex: index,
    renderMode: 'restricted_js_block',
    canEnterRestrictedJsRuntime: true,
    runtimeKind: 'worker',
    runtimeEntry: 'restricted-block-runtime.worker',
    contributionCode: 'qa.runtime-orchestration',
    sourceStatus: 'ready',
    source_sha256: sha256Text(source),
    catalogId: 'qa:runtime-orchestration',
    runPlan: {
      ok: true,
      request: {
        requestId: `qa:${blockId}:${codeRef}`,
        blockId,
        program: {
          kind: 'source',
          source,
          sourceSha256: sha256Text(source),
          allowedImports: [
            '@1flowbase/block-sdk',
            '@1flowbase/block-renderer/antd-facade'
          ]
        },
        props: { blockId },
        state: {},
        contextSnapshot: {
          page: { id: 'runtime-fixture', route: '/runtime-fixture' }
        },
        limits: { timeoutMs: 3000, maxRenderDepth: 8, maxRenderNodes: 250 }
      },
      schemaValidationOptions: {
        maxDepth: 8,
        maxNodes: 250,
        allowedDataPermissions: ['query'],

        allowedEvents: []
      },
      mediatorPolicy: {
        allowedEvents: [],
        maxEventChainDepth: 4
      }
    }
  };
}

function RuntimeOrchestrationFixture() {
  const sourceItems = useMemo(
    () =>
      Array.from({ length: blockCount }, (_, index) =>
        createRunPlanItem(index)
      ),
    []
  );
  const [items, setItems] = useState(sourceItems);
  const didRecordSourceFetchRef = useRef(false);
  const [lookupStatus, setLookupStatus] = useState<'ready' | 'pending'>(() =>
    sessionStorage.getItem('runtime-fixture-mode') === 'l2' ? 'pending' : 'ready'
  );
  const [storageScan, setStorageScan] = useState<'idle' | 'clean' | 'canary'>('idle');
  useEffect(() => {
    if (didRecordSourceFetchRef.current) return;
    didRecordSourceFetchRef.current = true;
    for (const item of sourceItems) {
      recordFrontstageRuntimeObservation({
        stage: 'source_fetch',
        cacheTier: 'network',
        actorId: 'qa-actor',
        workspaceId: 'qa',
        pageId: 'runtime-fixture',
        tabId: null,
        blockId: item.blockId
      });
    }
  }, [sourceItems]);
  useEffect(() => {
    if (lookupStatus !== 'pending') return;
    let active = true;
    void Promise.all(
      sourceItems.map(async (item) => {
        const lookup = await frontstageCompiledArtifactCache.get({
          actorId: 'qa-actor',
          workspaceId: 'qa',
          runtimeFingerprint: getFrontstageRestrictedBlockRuntimeFingerprint(),
          sourceSha256: item.source_sha256
        });
        return lookup.status === 'hit'
          ? {
              ...item,
              runPlan: {
                ...item.runPlan,
                request: {
                  ...item.runPlan.request,
                  program: {
                    kind: 'compiled_artifact' as const,
                    artifact: lookup.artifact,
                    sourceSha256: item.source_sha256,
                    fallback: item.runPlan.request.program.kind === 'source'
                      ? item.runPlan.request.program
                      : item.runPlan.request.program.fallback
                  }
                }
              }
            }
          : item;
      })
    ).then((nextItems) => {
      if (!active) return;
      sessionStorage.removeItem('runtime-fixture-mode');
      setItems(nextItems);
      setLookupStatus('ready');
    });
    return () => {
      active = false;
    };
  }, [lookupStatus, sourceItems]);
  const runtimeRunPlanState = useMemo<FrontstagePageCanvasRuntimeRunPlanState>(
    () => ({
      workspaceId: 'qa',
      pageId: 'runtime-fixture',
      items: lookupStatus === 'pending' ? [] : items
    }),
    [items, lookupStatus]
  );
  const content = useMemo(
    () =>
      createFrontstagePageContentFixture({
        page: {
          id: 'runtime-fixture',
          title: `Runtime fixture · ${blockCount} blocks`
        },
        root: {
          uid: 'runtime-fixture-root',
          payload: {
            blocks: sourceItems.map((item, index) => ({
              id: item.blockId,
              renderer_version: 'v1',
              codeRef: item.codeRef,
              contributionCode: item.contributionCode,
              runtime: { kind: 'worker', entry: item.runtimeEntry },
              layout: { order: index, region: 'main', span: 12 }
            }))
          }
        }
      }),
    [sourceItems]
  );
  const [demands, setDemands] = useState<
    Record<string, FrontstageRuntimeDemandPriority>
  >({});
  const [stats, setStats] = useState({ created: 0, active: 0, maxActive: 0 });
  const activeLeases = useRef(
    new Set<FrontstageRestrictedBlockRuntimeSession>()
  );
  const pendingArtifactWrites = useRef(new Set<Promise<unknown>>());
  const artifactCache = useMemo(
    () => ({
      put(...args: Parameters<typeof frontstageCompiledArtifactCache.put>) {
        const pending = frontstageCompiledArtifactCache.put(...args);
        pendingArtifactWrites.current.add(pending);
        void pending.finally(() => pendingArtifactWrites.current.delete(pending));
        return pending;
      }
    }),
    []
  );

  const runtimeSessionFactory = useCallback(
    (options: FrontstageRestrictedBlockRuntimeHostOptions) => {
      const session = createFrontstageRestrictedBlockRuntimeSession(options);
      activeLeases.current.add(session);
      setStats((current) => {
        const active = current.active + 1;
        return {
          created: current.created + 1,
          active,
          maxActive: Math.max(current.maxActive, active)
        };
      });
      let settled = false;
      const settle = () => {
        if (settled) return;
        settled = true;
        activeLeases.current.delete(session);
        setStats((current) => ({
          ...current,
          active: Math.max(0, current.active - 1)
        }));
      };
      return {
        ...session,
        subscribe(listener) {
          return session.subscribe((snapshot) => {
            if (snapshot.status !== 'running') settle();
            listener(snapshot);
          });
        },
        dispose() {
          settle();
          return session.dispose();
        }
      } satisfies FrontstageRestrictedBlockRuntimeSession;
    },
    []
  );

  const sessions = useFrontstagePageCanvasRuntimeSessions({
    actorId: 'qa-actor',
    actorWorkspaceId: 'qa',
    runtimeRunPlanState,
    runtimeSessionFactory,
    artifactCache,
    demandsByBlockId: demands,
    maxConcurrent: 2,
    handlers: {
      interface: (message) =>
        nonSettlingBlock &&
        message.requestId === 'qa:runtime-fixture-1:runtime-fixture-1-code'
          ? new Promise(() => {})
          : new Promise((resolve) =>
            setTimeout(
              () =>
                resolve({
                  ok: true,
                  sequence: Date.now(),
                  token: 'runtime-secret-canary',
                  headers: { authorization: 'runtime-secret-canary' },
                  response: 'runtime-secret-canary',
                  result: 'runtime-secret-canary',
                  log: 'runtime-secret-canary',
                  effect: 'runtime-secret-canary',
                  interface: 'runtime-secret-canary'
                }),
              180
            )
            )
    }
  });
  const ready = sessions.entries.filter(
    (entry) => entry.status === 'ready'
  ).length;
  const failed = sessions.entries.filter(
    (entry) =>
      entry.status === 'failed' ||
      entry.status === 'timed_out' ||
      entry.status === 'factory_failed'
  ).length;
  const errorKinds = sessions.entries.flatMap((entry) =>
    'snapshot' in entry && entry.snapshot.error
      ? [entry.snapshot.error.kind]
      : []
  );
  const observationStages = readFrontstageRuntimeObservations().map(
    (entry) => `${entry.stage}:${entry.cacheTier}:${entry.count}`
  );

  const reload = async (mode: 'cold' | 'l2') => {
    await Promise.allSettled([...pendingArtifactWrites.current]);
    clearFrontstageRuntimeSessionCache();
    resetFrontstageRuntimeObservations();
    if (mode === 'cold') {
      await frontstageCompiledArtifactCache.deleteActor('qa-actor');
      sessionStorage.removeItem('runtime-fixture-mode');
    } else {
      sessionStorage.setItem('runtime-fixture-mode', 'l2');
    }
    window.location.reload();
  };

  const scanStorage = async () => {
    const records = await createIndexedDbArtifactCacheStore().list();
    setStorageScan(
      JSON.stringify(records).includes('runtime-secret-canary')
        ? 'canary'
        : 'clean'
    );
  };

  return (
    <div style={{ padding: 16 }}>
      <div
        data-testid="runtime-fixture-stats"
        data-created={stats.created}
        data-active={stats.active}
        data-max-active={stats.maxActive}
        data-ready={ready}
        data-failed={failed}
        data-error-kinds={errorKinds.join(',')}
        data-lookup-status={lookupStatus}
        data-observation-stages={observationStages.join(',')}
        data-storage-scan={storageScan}
        data-ready-signal={
          lookupStatus === 'ready' &&
          ready + failed === blockCount &&
          stats.active === 0
            ? 'settled'
            : 'pending'
        }
        style={{
          position: 'sticky',
          top: 0,
          zIndex: 100,
          background: '#fff',
          padding: 8
        }}
      >
        created={stats.created} active={stats.active} max={stats.maxActive}{' '}
        ready={ready} failed={failed}
        <button data-testid="runtime-fixture-cold" onClick={() => void reload('cold')}>
          Cold run
        </button>
        <button data-testid="runtime-fixture-l2" onClick={() => void reload('l2')}>
          L2 reload
        </button>
        <button data-testid="runtime-fixture-storage-scan" onClick={() => void scanStorage()}>
          Scan storage
        </button>
      </div>
      <PageCanvas
        content={content}
        runtimeRunPlanState={runtimeRunPlanState}
        runtimeSessionEntries={sessions.entries}
        onRuntimeDemandChange={(blockId, priority) =>
          setDemands((current) =>
            current[blockId] === priority
              ? current
              : { ...current, [blockId]: priority }
          )
        }
        onRuntimeRetry={sessions.retryBlock}
      />
    </div>
  );
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ConfigProvider>
      <RuntimeOrchestrationFixture />
    </ConfigProvider>
  </StrictMode>
);
