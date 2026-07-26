import '@ant-design/v5-patch-for-react-19';
import { Button, ConfigProvider, Select } from 'antd';
import type { BlockContext } from '@1flowbase/page-protocol';
import type { ComponentType } from 'react';
import { StrictMode, useEffect, useMemo, useState } from 'react';
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
  subscribeFrontstageRuntimeObservations
} from '../../lib/page-canvas/runtime-observation';

type FixtureBlockProps = {
  label: string;
};

let firstThrowPending = false;

function FirstBlock({
  props,
  ctx
}: {
  props: FixtureBlockProps;
  ctx: BlockContext;
}) {
  const count = Number(ctx.inputs.count ?? 0);
  if (firstThrowPending) {
    firstThrowPending = false;
    throw new Error('controlled Native render failure');
  }
  return (
    <div data-testid="native-fixture-first-output" className="shared-name">
      <style>{`.shared-name { color: rgb(22, 119, 255); }`}</style>
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
  const [count, setCount] = useState(0);
  return (
    <div data-testid="native-fixture-second-output" className="shared-name">
      <style>{`.shared-name { color: rgb(82, 196, 26); }`}</style>
      adjacent:{count}
      <Button
        onClick={() =>
          setCount((value) => {
            const next = value + 1;
            ctx.outputs.publish({ total: next });
            return next;
          })
        }
      >
        input update
      </Button>
    </div>
  );
}

const components = {
  first: FirstBlock,
  second: SecondBlock
} satisfies Record<string, ComponentType<any>>;

function NativeReactTrialFixture() {
  const [sourceRevision, setSourceRevision] = useState(1);
  const [demands, setDemands] = useState<Record<string, 0 | 1 | 2 | 3>>({
    first: 1,
    second: 1
  });
  const [preparationFailure, setPreparationFailure] = useState(false);
  const [hidden, setHidden] = useState(false);
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
      )
    ],
    [demands.first, demands.second, preparationFailure, sourceRevision]
  );
  const [observations, setObservations] = useState(() =>
    readFrontstageRuntimeObservations()
  );
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
          render throw once
        </button>
        <button onClick={() => setPreparationFailure(true)}>
          compile failure
        </button>
        <button onClick={() => setHidden((value) => !value)}>
          hidden page
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
      <PageCanvas
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
    </main>
  );
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
    runtime: { kind: 'iframe', entry: `blocks/${id}.js` },
    layout: { order, region: 'main', span: 12 },
    props,
    ports
  };
}

function preparation(
  blockId: 'first' | 'second',
  slotIndex: number,
  component: ComponentType<any>,
  priority: 0 | 1 | 2 | 3,
  sourceRevision: number,
  failed: boolean,
  artifactCacheTier: 'l2' | 'miss'
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
    artifactCacheTier
  };
  return {
    ...base,
    status: 'ready',
    prepared,
    mountIntent: priority <= 1 ? { blockId, slotIndex, identityInput } : null
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
