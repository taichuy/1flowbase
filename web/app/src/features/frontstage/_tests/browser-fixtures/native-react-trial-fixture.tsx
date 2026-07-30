import '@ant-design/v5-patch-for-react-19';
import { Button, ConfigProvider, Select } from 'antd';
import { UserOutlined } from '@ant-design/icons';
import { EChart } from '@1flowbase/charts';
import { Surface } from '@1flowbase/native-components';
import { MarkdownEditor, MarkdownPreview } from '@1flowbase/rich-text';
import nativeComponentsCss from '@1flowbase/native-components/styles.css?raw';
import richTextCss from '@1flowbase/rich-text/styles.css?raw';
import vditorCss from 'vditor/dist/index.css?raw';
import type { BlockContext } from '@1flowbase/page-protocol';
import type { ComponentProps, ComponentType } from 'react';
import {
  StrictMode,
  useEffect,
  useLayoutEffect,
  useMemo,
  useState
} from 'react';
import { createRoot } from 'react-dom/client';

import { PageCanvas } from '../../components/PageCanvas';
import { createFrontstagePageContentFixture } from '../frontstage-page-content-fixtures';
import type {
  FrontstageNativePreparedRuntime,
  FrontstageNativePreparationSnapshot
} from '../../lib/page-canvas/native-runtime-preparation';
import {
  readFrontstageRuntimeObservations,
  resetFrontstageRuntimeObservations,
  subscribeFrontstageRuntimeObservations,
  type FrontstageRuntimeObservation
} from '../../lib/page-canvas/runtime-observation';

type FixtureBlockProps = {
  label: string;
};

let firstThrowPending = false;
const fixtureCounters = {
  pageCanvasRenders: 0,
  firstRenders: 0,
  secondRenders: 0,
  firstMounts: 0,
  firstUnmounts: 0,
  secondMounts: 0,
  secondUnmounts: 0,
  publishStarted: 0,
  publishCompleted: 0,
  publishLastOutcome: 'idle',
  nextHookIdentity: 0
};

function syncFixtureCounters() {
  const stats = document.querySelector<HTMLElement>(
    '[data-testid="native-frontstage-stats"]'
  );
  if (!stats) return;
  stats.dataset.pageCanvasRenders = String(fixtureCounters.pageCanvasRenders);
  stats.dataset.firstRenders = String(fixtureCounters.firstRenders);
  stats.dataset.secondRenders = String(fixtureCounters.secondRenders);
  stats.dataset.firstMounts = String(fixtureCounters.firstMounts);
  stats.dataset.firstUnmounts = String(fixtureCounters.firstUnmounts);
  stats.dataset.secondMounts = String(fixtureCounters.secondMounts);
  stats.dataset.secondUnmounts = String(fixtureCounters.secondUnmounts);
  stats.dataset.publishStarted = String(fixtureCounters.publishStarted);
  stats.dataset.publishCompleted = String(fixtureCounters.publishCompleted);
  stats.dataset.publishLastOutcome = fixtureCounters.publishLastOutcome;
}

function useFixtureBlockCounters(block: 'first' | 'second') {
  const [hookIdentity] = useState(
    () => `${block}-hook-${++fixtureCounters.nextHookIdentity}`
  );
  if (block === 'first') {
    fixtureCounters.firstRenders += 1;
  } else {
    fixtureCounters.secondRenders += 1;
  }
  useLayoutEffect(syncFixtureCounters);
  useEffect(() => {
    if (block === 'first') {
      fixtureCounters.firstMounts += 1;
    } else {
      fixtureCounters.secondMounts += 1;
    }
    syncFixtureCounters();
    return () => {
      if (block === 'first') {
        fixtureCounters.firstUnmounts += 1;
      } else {
        fixtureCounters.secondUnmounts += 1;
      }
      syncFixtureCounters();
    };
  }, [block]);
  return hookIdentity;
}

function FirstBlock({
  props,
  ctx
}: {
  props: FixtureBlockProps;
  ctx: BlockContext;
}) {
  const hookIdentity = useFixtureBlockCounters('first');
  const count = Number(ctx.inputs.count ?? 0);
  if (firstThrowPending) {
    throw new Error('controlled Native render failure');
  }
  return (
    <div
      data-testid="native-fixture-first-output"
      data-hook-identity={hookIdentity}
      data-render-count={fixtureCounters.firstRenders}
      className="shared-name"
    >
      <style>{`:host { --native-fixture-tone: rgb(22, 119, 255); }
        @keyframes native-fixture-pulse { from { opacity: 0.99; } to { opacity: 1; } }
        .shared-name { color: var(--native-fixture-tone); animation: native-fixture-pulse 1s; }`}</style>
      {props.label}:{count}
      <Button data-testid="native-fixture-local-button">local</Button>
      <Select
        open
        value="first"
        options={[{ value: 'first', label: 'shadow-contained-popup' }]}
      />
    </div>
  );
}

function SecondBlock({ ctx }: { ctx: BlockContext }) {
  const hookIdentity = useFixtureBlockCounters('second');
  const [count, setCount] = useState(0);
  return (
    <div
      data-testid="native-fixture-second-output"
      data-hook-identity={hookIdentity}
      data-render-count={fixtureCounters.secondRenders}
      className="shared-name"
    >
      <style>{`:host { --native-fixture-tone: rgb(82, 196, 26); }
        .shared-name { color: var(--native-fixture-tone); }`}</style>
      adjacent:{count}
      <Button
        onClick={() => {
          const next = count + 1;
          setCount(next);
          fixtureCounters.publishStarted += 1;
          syncFixtureCounters();
          try {
            const result = ctx.outputs.publish({ total: next });
            if (result instanceof Promise) {
              fixtureCounters.publishLastOutcome = 'promise';
              void result.then(
                (settled) => {
                  if (settled.ok) {
                    fixtureCounters.publishLastOutcome = 'ok';
                  } else if (settled.stale) {
                    fixtureCounters.publishLastOutcome = 'stale';
                  } else {
                    fixtureCounters.publishLastOutcome = 'rejected';
                  }
                  syncFixtureCounters();
                },
                () => {
                  fixtureCounters.publishLastOutcome = 'promise-rejected';
                  syncFixtureCounters();
                }
              );
            } else if (result.ok) {
              fixtureCounters.publishLastOutcome = 'ok';
            } else if (result.stale) {
              fixtureCounters.publishLastOutcome = 'stale';
            } else {
              fixtureCounters.publishLastOutcome = 'rejected';
            }
          } finally {
            fixtureCounters.publishCompleted += 1;
            syncFixtureCounters();
          }
        }}
      >
        input update
      </Button>
    </div>
  );
}

function PublicModulesBlock({ props }: { props: FixtureBlockProps }) {
  const [markdown, setMarkdown] = useState(`# ${props.label}`);
  return (
    <Surface
      aria-label={`public-modules-${props.label}`}
      data-testid={`public-modules-${props.label}`}
    >
      <h3>
        <UserOutlined /> {props.label}
      </h3>
      <EChart
        ariaLabel={`chart-${props.label}`}
        option={{
          xAxis: { type: 'category', data: ['A', 'B'] },
          yAxis: { type: 'value' },
          series: [{ type: 'bar', data: [3, 7] }]
        }}
        style={{ height: 140 }}
      />
      <MarkdownEditor
        ariaLabel={`editor-${props.label}`}
        height={180}
        value={markdown}
        onChange={setMarkdown}
      />
      <MarkdownPreview aria-label={`preview-${props.label}`} value={markdown} />
    </Surface>
  );
}

const components = {
  first: FirstBlock,
  second: SecondBlock,
  publicA: PublicModulesBlock,
  publicB: PublicModulesBlock
} satisfies Record<string, ComponentType<any>>;

const publicModuleAssets = [
  fixtureModuleStyle('@1flowbase/native-components', 'c', nativeComponentsCss),
  fixtureModuleStyle(
    '@1flowbase/rich-text',
    'd',
    `${vditorCss}\n${richTextCss}`
  )
];

function NativeReactTrialFixture() {
  const [sourceRevision, setSourceRevision] = useState(1);
  const [demands, setDemands] = useState<Record<string, 0 | 1 | 2 | 3>>({
    first: 1,
    second: 1
  });
  const [preparationFailure, setPreparationFailure] = useState(false);
  const [hidden, setHidden] = useState(false);
  const [pageMounted, setPageMounted] = useState(true);
  const content = useMemo(
    () =>
      createFrontstagePageContentFixture({
        page: {
          id: 'native-frontstage-fixture',
          title: 'Native Frontstage fixture'
        },
        root: {
          payload: {
            blocks: [
              fixtureBlock(
                'first',
                0,
                { label: `source-${sourceRevision}` },
                {
                  inputs: [
                    {
                      name: 'count',
                      schema: { type: 'integer' },
                      source: {
                        block_id: 'second',
                        output: 'total',
                        scope: 'tab'
                      }
                    }
                  ],
                  outputs: []
                }
              ),
              fixtureBlock(
                'second',
                1,
                {},
                {
                  inputs: [],
                  outputs: [{ name: 'total', schema: { type: 'integer' } }]
                }
              ),
              fixtureBlock(
                'public-a',
                2,
                { label: 'a' },
                { inputs: [], outputs: [] }
              ),
              fixtureBlock(
                'public-b',
                3,
                { label: 'b' },
                { inputs: [], outputs: [] }
              )
            ]
          }
        }
      }),
    [sourceRevision]
  );
  const preparations = useMemo(
    () => [
      preparation(
        'first',
        0,
        components.first,
        demands.first,
        sourceRevision,
        preparationFailure,
        'l2'
      ),
      preparation(
        'second',
        1,
        components.second,
        demands.second,
        1,
        false,
        'miss'
      ),
      preparation(
        'public-a',
        2,
        components.publicA,
        1,
        1,
        false,
        'l2',
        publicModuleAssets
      ),
      preparation(
        'public-b',
        3,
        components.publicB,
        1,
        1,
        false,
        'l2',
        publicModuleAssets
      )
    ],
    [demands.first, demands.second, preparationFailure, sourceRevision]
  );
  const [observations, setObservations] = useState<
    readonly FrontstageRuntimeObservation[]
  >(() => readFrontstageRuntimeObservations());
  useEffect(() => subscribeFrontstageRuntimeObservations(setObservations), []);

  const retryPreparation = (blockId: string) => {
    if (blockId === 'first' && preparationFailure) {
      setPreparationFailure(false);
      setSourceRevision((value) => value + 1);
    }
  };

  return (
    <main style={{ padding: 16 }} data-hidden-page={hidden}>
      <div
        data-testid="native-frontstage-stats"
        data-source-revision={sourceRevision}
        data-page-mounted={pageMounted ? 'true' : 'false'}
        data-page-canvas-renders={fixtureCounters.pageCanvasRenders}
        data-first-renders={fixtureCounters.firstRenders}
        data-second-renders={fixtureCounters.secondRenders}
        data-first-mounts={fixtureCounters.firstMounts}
        data-first-unmounts={fixtureCounters.firstUnmounts}
        data-second-mounts={fixtureCounters.secondMounts}
        data-second-unmounts={fixtureCounters.secondUnmounts}
        data-publish-started={fixtureCounters.publishStarted}
        data-publish-completed={fixtureCounters.publishCompleted}
        data-publish-last-outcome={fixtureCounters.publishLastOutcome}
        data-max-concurrent="1"
        data-demands={`${demands.first},${demands.second}`}
        data-observation-stages={observations
          .map(({ stage }) => stage)
          .join(',')}
        data-instance-epochs={observations
          .flatMap(({ instanceEpoch }) =>
            instanceEpoch ? [instanceEpoch] : []
          )
          .join(',')}
        data-ready-signal={
          preparations.every(({ status }) => status === 'ready')
            ? 'settled'
            : 'pending'
        }
      >
        <button onClick={() => setSourceRevision((value) => value + 1)}>
          source remount
        </button>
        <button
          onClick={() => {
            firstThrowPending = true;
            setSourceRevision((value) => value + 1);
          }}
        >
          render failure
        </button>
        <button
          onClick={() => {
            firstThrowPending = false;
          }}
        >
          allow render recovery
        </button>
        <button onClick={() => setPreparationFailure(true)}>
          compile failure
        </button>
        <button onClick={() => setHidden((value) => !value)}>
          hidden page
        </button>
        <button onClick={() => setPageMounted((value) => !value)}>
          {pageMounted ? 'exit page' : 'enter page'}
        </button>
        <button onClick={() => resetFrontstageRuntimeObservations()}>
          reset observations
        </button>
        {[0, 1, 2, 3].map((priority) => (
          <button
            key={priority}
            onClick={() =>
              setDemands((current) => ({
                ...current,
                first: priority as 0 | 1 | 2 | 3
              }))
            }
          >
            demand {priority}
          </button>
        ))}
      </div>
      {pageMounted ? (
        <InstrumentedPageCanvas
          content={content}
          runtimePreparations={hidden ? [] : preparations}
          runtimeContext={{
            currentUser: { id: 'fixture-user', displayName: 'Fixture User' },
            workspace: { id: 'fixture-workspace' },
            application: null,
            theme: { mode: 'light', tokens: {} },
            ui: {}
          }}
          onRuntimeDemandChange={(blockId, priority) =>
            setDemands((current) =>
              current[blockId] === priority
                ? current
                : { ...current, [blockId]: priority }
            )
          }
          onRuntimeRetry={retryPreparation}
        />
      ) : null}
    </main>
  );
}

function InstrumentedPageCanvas(props: ComponentProps<typeof PageCanvas>) {
  fixtureCounters.pageCanvasRenders += 1;
  useLayoutEffect(syncFixtureCounters);
  return <PageCanvas {...props} />;
}

function fixtureBlock(
  id: string,
  order: number,
  props: Record<string, unknown>,
  ports: Record<string, unknown>
) {
  return {
    id,
    renderer_version: 'v1',
    codeRef: `${id}-code`,
    contributionCode: `qa.native.${id}`,
    runtime: { kind: 'native_react', entry: `blocks/${id}.js` },
    layout: { order, region: 'main', span: 12 },
    props,
    ports
  };
}

function preparation(
  blockId: 'first' | 'second' | 'public-a' | 'public-b',
  slotIndex: number,
  component: ComponentType<any>,
  priority: 0 | 1 | 2 | 3,
  sourceRevision: number,
  failed: boolean,
  artifactCacheTier: 'l2' | 'miss',
  moduleAssets: FrontstageNativePreparedRuntime['moduleAssets'] = []
): FrontstageNativePreparationSnapshot {
  const base = {
    blockId,
    slotIndex,
    priority,
    generation: sourceRevision,
    observationContext: {
      actorId: 'fixture-user',
      workspaceId: 'fixture-workspace',
      pageId: 'native-frontstage-fixture',
      tabId: 'tab-1',
      blockId
    }
  };
  if (failed) {
    return {
      ...base,
      status: 'failed',
      failedStage: 'compile',
      error: new Error('controlled compile failure')
    };
  }
  if (priority === 3) return { ...base, status: 'idle' };
  const identityInput = {
    sourceSha256: `${blockId}-${sourceRevision}`.padEnd(64, '0'),
    runtimeFingerprint: 'native-fixture-runtime',
    dependencyLockIdentity: 'fixture-lock'
  };
  const prepared: FrontstageNativePreparedRuntime = {
    artifact: {} as FrontstageNativePreparedRuntime['artifact'],
    component: component as FrontstageNativePreparedRuntime['component'],
    identityInput,
    artifactCacheTier,
    moduleAssets
  };
  return {
    ...base,
    status: 'ready',
    prepared,
    mountIntent: priority <= 1 ? { blockId, slotIndex, identityInput } : null
  };
}

function fixtureModuleStyle(
  moduleSource: string,
  digestCharacter: string,
  css: string
): FrontstageNativePreparedRuntime['moduleAssets'][number] {
  return {
    module_source: moduleSource,
    role: 'shadow_style',
    media_type: 'text/css; charset=utf-8',
    sha256: digestCharacter.repeat(64),
    url: `/fixture-assets/${moduleSource}/${digestCharacter}`,
    bytes: new TextEncoder().encode(css).buffer as ArrayBuffer
  };
}

const root = document.getElementById('root');
if (!root) throw new Error('Native Frontstage fixture root is missing.');
createRoot(root).render(
  <StrictMode>
    <ConfigProvider>
      <NativeReactTrialFixture />
    </ConfigProvider>
  </StrictMode>
);
