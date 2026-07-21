import '@ant-design/v5-patch-for-react-19';
import { ConfigProvider } from 'antd';
import { StrictMode, useCallback, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';

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
  const response = await ctx.interfaces.call({
    interfaceId: 'list_records',
    schemaDigest: 'digest-list-records'
  }, {
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
    catalogId: 'qa:runtime-orchestration',
    runPlan: {
      ok: true,
      request: {
        requestId: `qa:${blockId}:${codeRef}`,
        blockId,
        source,
        props: { blockId },
        state: {},
        contextSnapshot: {
          page: { id: 'runtime-fixture', route: '/runtime-fixture' }
        },
        limits: { timeoutMs: 3000, maxRenderDepth: 8, maxRenderNodes: 250 },
        allowedImports: [
          '@1flowbase/block-sdk',
          '@1flowbase/block-renderer/antd-facade'
        ]
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
  const items = useMemo(
    () =>
      Array.from({ length: blockCount }, (_, index) =>
        createRunPlanItem(index)
      ),
    []
  );
  const runtimeRunPlanState = useMemo<FrontstagePageCanvasRuntimeRunPlanState>(
    () => ({ workspaceId: 'qa', pageId: 'runtime-fixture', items }),
    [items]
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
            blocks: items.map((item, index) => ({
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
    [items]
  );
  const [demands, setDemands] = useState<
    Record<string, FrontstageRuntimeDemandPriority>
  >({});
  const [stats, setStats] = useState({ created: 0, active: 0, maxActive: 0 });
  const activeLeases = useRef(
    new Set<FrontstageRestrictedBlockRuntimeSession>()
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
    runtimeRunPlanState,
    runtimeSessionFactory,
    demandsByBlockId: demands,
    maxConcurrent: 2,
    handlers: {
      interface: (message) =>
        nonSettlingBlock &&
        message.requestId === 'qa:runtime-fixture-1:runtime-fixture-1-code'
          ? new Promise(() => {})
          : new Promise((resolve) =>
              setTimeout(() => resolve({ ok: true }), 180)
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
