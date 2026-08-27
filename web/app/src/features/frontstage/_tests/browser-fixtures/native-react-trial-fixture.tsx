import { Button, ConfigProvider, Select } from 'antd';
import { UserOutlined } from '@ant-design/icons';
import { EChart } from '@1flowbase/charts';
import { Surface } from '@1flowbase/native-components';
import { VditorEditor } from '@1flowbase/rich-text';
import nativeComponentsCss from '@1flowbase/native-components/styles.css?raw';
import richTextCss from '@1flowbase/rich-text/styles.css?raw';
import vditorCss from 'vditor/dist/index.css?raw';
import type { BlockContext } from '@1flowbase/page-protocol';
import type { IsolatedFrontendBlockCapabilityHandlers } from '@1flowbase/page-runtime';
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
import type { FrontstageBlockInstance } from '../../lib/page-document';
import type { FrontstageBlockPorts } from '../../lib/page-signals/types';
import { createFrontstageNativeReactModuleRegistry } from '../../lib/native-trusted-block-runtime-factory';
import type { PreparedFrontstageIsolatedContribution } from '../../lib/isolated-frontend-block-contribution';

const ISOLATED_FIXTURE_SOURCE = `globalThis.__oneflowbaseIsolatedBlock = {
  mount(root, props, capabilities) {
    let tick = 0;
    root.dataset.label = String(props.label);
    const timer = setInterval(() => {
      tick += 1;
      root.dataset.tick = String(tick);
      capabilities.request('block.output.publish', { output: 'tick', value: tick }).catch(() => {});
    }, 25);
    return {
      update(nextProps) { root.dataset.label = String(nextProps.label); },
      dispose() { clearInterval(timer); }
    };
  }
};`;

type FixtureBlockProps = {
  label: string;
};

type FixtureComponent =
  | ComponentType<{ props: FixtureBlockProps; ctx: BlockContext }>
  | ComponentType<{ ctx: BlockContext }>
  | ComponentType<{ props: FixtureBlockProps }>;

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
  isolatedMessages: 0,
  isolatedLastTick: 0,
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
  stats.dataset.isolatedMessages = String(fixtureCounters.isolatedMessages);
  stats.dataset.isolatedLastTick = String(fixtureCounters.isolatedLastTick);
  stats.dataset.isolatedReadySignal =
    fixtureCounters.isolatedMessages > 0 ? 'settled' : 'pending';
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

function PublicModulesBlock({
  props,
  ctx
}: {
  props: FixtureBlockProps;
  ctx: BlockContext;
}) {
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
      <VditorEditor
        api={ctx.api}
        ariaLabel={`editor-${props.label}`}
        height={260}
        value={markdown}
        onChange={setMarkdown}
      />
    </Surface>
  );
}

const components = {
  first: FirstBlock,
  second: SecondBlock,
  publicA: PublicModulesBlock,
  publicB: PublicModulesBlock
} satisfies Record<string, FixtureComponent>;

const publicModuleAssets = [
  fixtureModuleStyle('@1flowbase/native-components', 'c', nativeComponentsCss),
  fixtureModuleStyle(
    '@1flowbase/rich-text',
    'd',
    `${vditorCss}\n${richTextCss}`
  )
];

function CatalogIconsProbe() {
  const [Icon, setIcon] = useState<ComponentType<{
    'aria-label': string;
  }> | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    const registry = createFrontstageNativeReactModuleRegistry();

    void registry.load('@ant-design/icons').then(
      (module) => {
        if (disposed) return;
        if (typeof module.CheckCircleOutlined !== 'object') {
          setError('CheckCircleOutlined export is unavailable');
          return;
        }
        setIcon(
          () =>
            module.CheckCircleOutlined as ComponentType<{
              'aria-label': string;
            }>
        );
      },
      (reason) => {
        if (!disposed)
          setError(reason instanceof Error ? reason.message : String(reason));
      }
    );

    return () => {
      disposed = true;
    };
  }, []);

  if (error) {
    return (
      <div data-testid="catalog-icons-probe" data-status="failed">
        {error}
      </div>
    );
  }
  if (!Icon) {
    return <div data-testid="catalog-icons-probe" data-status="loading" />;
  }
  return (
    <div data-testid="catalog-icons-probe" data-status="ready">
      <Icon aria-label="catalog-check-circle-icon" />
    </div>
  );
}

function CatalogRichTextProbe() {
  const [Editor, setEditor] = useState<ComponentType<{
    value: string;
    onChange(value: string): void;
    ariaLabel: string;
  }> | null>(null);
  const [value, setValue] = useState('# Catalog ready');
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    const registry = createFrontstageNativeReactModuleRegistry();

    void registry.load('@1flowbase/rich-text').then(
      (module) => {
        if (disposed) return;
        if (typeof module.VditorEditor !== 'function') {
          setError('VditorEditor export is unavailable');
          return;
        }
        setEditor(
          () =>
            module.VditorEditor as ComponentType<{
              value: string;
              onChange(value: string): void;
              ariaLabel: string;
            }>
        );
      },
      (reason) => {
        if (!disposed)
          setError(reason instanceof Error ? reason.message : String(reason));
      }
    );

    return () => {
      disposed = true;
    };
  }, []);

  if (error) {
    return (
      <div data-testid="catalog-rich-text-probe" data-status="failed">
        {error}
      </div>
    );
  }
  if (!Editor) {
    return <div data-testid="catalog-rich-text-probe" data-status="loading" />;
  }
  return (
    <div data-testid="catalog-rich-text-probe" data-status="ready">
      <Editor
        ariaLabel="catalog-rich-text-editor"
        value={value}
        onChange={setValue}
      />
    </div>
  );
}

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
              ),
              {
                id: 'isolated',
                renderer_version: 'v1',
                contributionCode: 'qa.isolated.timer',
                runtime: {
                  kind: 'isolated_iframe',
                  entry: '@fixture/isolated-timer'
                },
                layout: { order: 4, region: 'main', span: 12 },
                props: { label: `isolated-${sourceRevision}` },
                ports: { inputs: [], outputs: [] }
              }
            ]
          }
        }
      }),
    [sourceRevision]
  );
  const runtimeBlocks = useMemo<readonly FrontstageBlockInstance[]>(
    () => [
      fixtureRuntimeBlock(
        'first',
        0,
        { label: `source-${sourceRevision}` },
        {
          inputs: [
            {
              name: 'count',
              schema: { type: 'integer' },
              source: { block_id: 'second', output: 'total', scope: 'tab' }
            }
          ],
          outputs: []
        }
      ),
      fixtureRuntimeBlock(
        'second',
        1,
        {},
        {
          inputs: [],
          outputs: [{ name: 'total', schema: { type: 'integer' } }]
        }
      ),
      fixtureRuntimeBlock(
        'public-a',
        2,
        { label: 'a' },
        { inputs: [], outputs: [] }
      ),
      fixtureRuntimeBlock(
        'public-b',
        3,
        { label: 'b' },
        { inputs: [], outputs: [] }
      ),
      fixtureRuntimeBlock(
        'isolated',
        4,
        { label: `isolated-${sourceRevision}` },
        { inputs: [], outputs: [] },
        'isolated_iframe'
      ),
      ...Array.from({ length: 6 }, (_, index) =>
        fixtureRuntimeBlock(
          `filler-${index + 1}`,
          5 + index,
          {},
          {
            inputs: [],
            outputs: []
          }
        )
      )
    ],
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
  const isolatedPreparations = useMemo<
    readonly PreparedFrontstageIsolatedContribution[]
  >(
    () => [
      {
        state: 'prepared',
        blockInstanceId: 'isolated',
        contributionId: 'frontend-block.fixture.isolated-timer',
        blockId: 'fixture:isolated-timer',
        blockVersion: '1.0.0',
        graphFingerprint: 'fixture-isolated-graph',
        runtimeKind: 'isolated',
        executionKind: 'ui_mount',
        isolationRequirement: 'independent_realm',
        lifecycleKind: 'workspace_assignment',
        grantedPermissions: ['frontend-block.ui-mount.isolated-realm'],
        assetIntegrity: 'verified_sha256',
        program: {
          source: ISOLATED_FIXTURE_SOURCE,
          props: { label: `isolated-${sourceRevision}` }
        }
      }
    ],
    [sourceRevision]
  );
  const isolatedCapabilityHandlers = useMemo<
    Readonly<Record<string, IsolatedFrontendBlockCapabilityHandlers>>
  >(
    () => ({
      isolated: {
        'block.output.publish': (payload) => {
          fixtureCounters.isolatedMessages += 1;
          fixtureCounters.isolatedLastTick = readIsolatedTick(payload.value);
          syncFixtureCounters();
        }
      }
    }),
    []
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
        data-isolated-messages={fixtureCounters.isolatedMessages}
        data-isolated-last-tick={fixtureCounters.isolatedLastTick}
        data-isolated-ready-signal={
          fixtureCounters.isolatedMessages > 0 ? 'settled' : 'pending'
        }
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
        <button
          type="button"
          onClick={() => setSourceRevision((value) => value + 1)}
        >
          source remount
        </button>
        <button
          type="button"
          onClick={() => {
            firstThrowPending = true;
            setSourceRevision((value) => value + 1);
          }}
        >
          render failure
        </button>
        <button
          type="button"
          onClick={() => {
            firstThrowPending = false;
          }}
        >
          allow render recovery
        </button>
        <button type="button" onClick={() => setPreparationFailure(true)}>
          compile failure
        </button>
        <button type="button" onClick={() => setHidden((value) => !value)}>
          hidden page
        </button>
        <button type="button" onClick={() => setPageMounted((value) => !value)}>
          {pageMounted ? 'exit page' : 'enter page'}
        </button>
        <button
          type="button"
          onClick={() => resetFrontstageRuntimeObservations()}
        >
          reset observations
        </button>
        {[0, 1, 2, 3].map((priority) => (
          <button
            key={priority}
            type="button"
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
        <>
          <CatalogIconsProbe />
          <CatalogRichTextProbe />
          <div
            data-testid="issue-1896-scroll-owner"
            data-flowbase-frontstage-scroll-owner=""
            style={{ height: 560, overflow: 'auto', position: 'relative' }}
          >
            <InstrumentedPageCanvas
              content={content}
              runtimeBlocks={runtimeBlocks}
              runtimePreparations={hidden ? [] : preparations}
              isolatedRuntimePreparations={hidden ? [] : isolatedPreparations}
              isolatedCapabilityHandlersByBlockId={isolatedCapabilityHandlers}
              runtimeContext={{
                currentUser: {
                  id: 'fixture-user',
                  displayName: 'Fixture User'
                },
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
          </div>
        </>
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

function fixtureRuntimeBlock(
  id: string,
  order: number,
  props: Record<string, unknown>,
  ports: FrontstageBlockPorts,
  runtimeKind = 'native_react'
): FrontstageBlockInstance {
  return {
    id,
    rendererVersion: 'v1',
    sourceId: id,
    codeRef: `${id}-code`,
    sourceCodeRef: id,
    catalog: { providerCode: 'qa', installationId: 'fixture' },
    contribution: {
      pluginId: 'qa.native',
      pluginVersion: '1.0.0',
      code: id
    },
    runtime: {
      kind: runtimeKind,
      entry:
        runtimeKind === 'isolated_iframe'
          ? '@fixture/isolated-timer'
          : `blocks/${id}.js`,
      hint: runtimeKind
    },
    layout: { order, region: 'main', span: 12 },
    presentation: { heightMode: 'auto', height: null },
    order,
    props,
    ports
  };
}

const accumulatedFixtureMounts = new Set<string>();

function preparation(
  blockId: 'first' | 'second' | 'public-a' | 'public-b',
  slotIndex: number,
  component: FixtureComponent,
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
  const accumulatedMount = accumulatedFixtureMounts.has(blockId);
  if (priority === 3 && !accumulatedMount) return { ...base, status: 'idle' };
  const identityInput = {
    sourceSha256: `${blockId}-${sourceRevision}`.padEnd(64, '0'),
    compilerAbi: 'native-fixture-compiler',
    runtimeAbi: 'native-fixture-runtime'
  };
  const prepared: FrontstageNativePreparedRuntime = {
    artifact: {} as FrontstageNativePreparedRuntime['artifact'],
    component: component as FrontstageNativePreparedRuntime['component'],
    identityInput,
    artifactCacheTier,
    moduleAssets,
    moduleSources: []
  };
  if (priority <= 1) accumulatedFixtureMounts.add(blockId);
  return {
    ...base,
    status: 'ready',
    prepared,
    mountIntent:
      priority <= 1 || accumulatedMount
        ? { blockId, slotIndex, identityInput }
        : null
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

function readIsolatedTick(payload: unknown): number {
  if (typeof payload === 'number') return payload;
  throw new Error('Isolated fixture tick payload is invalid.');
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
