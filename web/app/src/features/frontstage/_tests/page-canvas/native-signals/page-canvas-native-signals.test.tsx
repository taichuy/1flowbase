import {
  act,
  fireEvent,
  render,
  waitFor,
  within
} from '@testing-library/react';
import { useState, type ComponentType } from 'react';
import { describe, expect, test, vi } from 'vitest';

import type {
  BlockContext,
  BlockContextOutputPublishResult
} from '@1flowbase/page-protocol';

import type { FrontstagePageContent } from '../../../api/page-content';
import {
  PageCanvas,
  type FrontstagePageCanvasRuntimeContext
} from '../../../components/PageCanvas';
import type {
  FrontstageNativePreparationSnapshot,
  FrontstageNativePreparedRuntime
} from '../../../lib/page-canvas/native-runtime-preparation';
import type { FrontstageNativeBlockContextHost } from '../../../lib/page-canvas/native-block-context-host';
import { createFrontstagePageContentFixture } from '../../frontstage-page-content-fixtures';

describe('PageCanvas Native Signal context', () => {
  test('D3-AC-003/005 publishes repeatedly, updates downstream without remount, and rejects the old remounted epoch', async () => {
    const producerContexts: BlockContext[] = [];
    const Producer = ({ ctx }: { ctx: BlockContext }) => {
      producerContexts.push(ctx);
      return <div data-testid="producer-ready">Producer</div>;
    };
    const Consumer = ({ ctx }: { ctx: BlockContext }) => {
      const [localCount, setLocalCount] = useState(0);
      return (
        <button
          data-testid="consumer-value"
          onClick={() => setLocalCount((value) => value + 1)}
        >
          {localCount}:{String(ctx.inputs.total ?? 'none')}
        </button>
      );
    };
    const content = pageContent();
    const runtimeContext: FrontstagePageCanvasRuntimeContext = {
      currentUser: { id: 'user-1', displayName: 'Ada' },
      workspace: { id: 'workspace-1', name: 'Workspace' },
      application: null,
      theme: { mode: 'light', tokens: { color: 'blue' } },
      ui: { locale: 'en_US' }
    };
    const mountedPreparations = [
      preparation('producer', 0, Producer),
      preparation('consumer', 1, Consumer)
    ];
    const view = render(
      <PageCanvas
        content={content}
        runtimeContext={runtimeContext}
        runtimePreparations={mountedPreparations}
      />
    );
    const producerRoot = await nativeRoot('producer');
    const consumerRoot = await nativeRoot('consumer');
    const consumerButton = await within(consumerRoot.shadow).findByTestId(
      'consumer-value'
    );
    fireEvent.click(consumerButton);
    expect(consumerButton).toHaveTextContent('1:none');
    expect(producerContexts.at(-1)).toMatchObject({
      currentUser: { id: 'user-1' },
      workspace: { id: 'workspace-1' },
      application: null,
      page: { id: 'page-1' },
      theme: { mode: 'light' },
      ui: { locale: 'en_US' }
    });

    const oldPublish = producerContexts.at(-1)!.outputs.publish;
    let firstPublish!: BlockContextOutputPublishResult;
    act(() => {
      firstPublish = oldPublish({
        total: 1
      }) as BlockContextOutputPublishResult;
    });
    expect(firstPublish).toMatchObject({ ok: true, stale: false });
    await waitFor(() => expect(consumerButton).toHaveTextContent('1:1'));
    act(() => {
      expect(oldPublish({ total: 2 })).toMatchObject({
        ok: true,
        stale: false
      });
    });
    await waitFor(() => expect(consumerButton).toHaveTextContent('1:2'));

    const contextsBeforeRemount = producerContexts.length;
    view.rerender(
      <PageCanvas
        content={content}
        runtimeContext={runtimeContext}
        runtimePreparations={[
          { ...mountedPreparations[0], mountIntent: null },
          mountedPreparations[1]
        ]}
      />
    );
    await waitFor(() =>
      expect(producerRoot.host.shadowRoot?.childNodes.length ?? 0).toBe(0)
    );
    await expect(
      Promise.resolve().then(() => oldPublish({ total: 3 }))
    ).resolves.toEqual({
      ok: false,
      stale: true
    });

    view.rerender(
      <PageCanvas
        content={content}
        runtimeContext={runtimeContext}
        runtimePreparations={mountedPreparations}
      />
    );
    await waitFor(() =>
      expect(producerContexts.length).toBeGreaterThan(contextsBeforeRemount)
    );
    const remountedPublish = producerContexts.at(-1)!.outputs.publish;
    expect(remountedPublish).not.toBe(oldPublish);
    act(() => {
      expect(remountedPublish({ total: 4 })).toMatchObject({
        ok: true,
        stale: false
      });
    });
    await waitFor(() => expect(consumerButton).toHaveTextContent('1:4'));
  });

  test('D4-AC-001/003 keeps the Native React instance mounted while concurrent API calls are pending or fail', async () => {
    const first = deferred<unknown>();
    const interfaceHandler = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockRejectedValueOnce(new Error('record query failed'));
    const observations: string[] = [];
    const diagnostics: string[] = [];
    const host: FrontstageNativeBlockContextHost = {
      interface: interfaceHandler,
      observeApiCall: ({ status }) => observations.push(status),
      reportDiagnostic: ({ message }) => diagnostics.push(message)
    };
    let renderCount = 0;
    const ApiBlock = ({ ctx }: { ctx: BlockContext }) => {
      const [localCount, setLocalCount] = useState(0);
      renderCount += 1;
      return (
        <div>
          <button
            data-testid="native-local-state"
            onClick={() => setLocalCount((value) => value + 1)}
          >
            {localCount}
          </button>
          <button
            data-testid="native-api-first"
            onClick={() => void ctx.api.get('/api/console/records/first')}
          >
            first
          </button>
          <button
            data-testid="native-api-second"
            onClick={() =>
              void ctx.api.get('/api/console/records/second').catch(() => undefined)
            }
          >
            second
          </button>
        </div>
      );
    };
    render(
      <PageCanvas
        content={pageContent()}
        runtimePreparations={[preparation('producer', 0, ApiBlock)]}
        nativeContextHost={host}
      />
    );
    const producerRoot = await nativeRoot('producer');
    const localState = await within(producerRoot.shadow).findByTestId(
      'native-local-state'
    );
    const initialRenderCount = renderCount;
    fireEvent.click(
      within(producerRoot.shadow).getByTestId('native-api-first')
    );
    fireEvent.click(
      within(producerRoot.shadow).getByTestId('native-api-second')
    );
    fireEvent.click(localState);

    expect(localState).toHaveTextContent('1');
    expect(renderCount).toBeGreaterThanOrEqual(initialRenderCount);
    expect(producerRoot.host.shadowRoot).not.toBeNull();
    await waitFor(() => expect(diagnostics).toEqual(['record query failed']));
    expect(observations.filter((status) => status === 'pending')).toHaveLength(2);

    first.resolve({ items: [] });
    await waitFor(() =>
      expect(observations.filter((status) => status === 'succeeded')).toHaveLength(1)
    );
    expect(localState).toHaveTextContent('1');
    expect(producerRoot.host.shadowRoot).not.toBeNull();
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

async function nativeRoot(blockId: string): Promise<{
  host: HTMLElement;
  shadow: HTMLElement;
}> {
  const selector = `[data-testid="frontstage-native-block-root-${blockId}"]`;
  await waitFor(() => expect(document.querySelector(selector)).not.toBeNull());
  const host = document.querySelector(selector) as HTMLElement;
  await waitFor(() => expect(host.shadowRoot).not.toBeNull());
  return {
    host,
    shadow: host.shadowRoot as unknown as HTMLElement
  };
}

function preparation(
  blockId: string,
  slotIndex: number,
  component: ComponentType<{ ctx: BlockContext }>
): Extract<FrontstageNativePreparationSnapshot, { status: 'ready' }> {
  const identityInput = {
    sourceSha256: blockId.padEnd(64, '0'),
    runtimeFingerprint: 'runtime-a',
    dependencyLockIdentity: 'lock-a'
  };
  return {
    status: 'ready',
    blockId,
    slotIndex,
    priority: 1,
    generation: 0,
    mountIntent: { blockId, slotIndex, identityInput },
    prepared: {
      artifact: {} as FrontstageNativePreparedRuntime['artifact'],
      component: component as FrontstageNativePreparedRuntime['component'],
      identityInput,
      artifactCacheTier: 'l2'
    }
  };
}

function pageContent(): FrontstagePageContent {
  const block = (
    id: string,
    order: number,
    ports: Record<string, unknown>
  ) => ({
    id,
    renderer_version: 'v1',
    codeRef: `${id}-code`,
    catalog: { providerCode: 'official', installationId: 'installation-1' },
    contribution: {
      pluginId: 'official.blocks',
      pluginVersion: '1.0.0',
      code: id
    },
    runtime: { kind: 'iframe', entry: `blocks/${id}.js` },
    layout: { order, region: 'main' },
    ports
  });
  return createFrontstagePageContentFixture({
    root: {
      uid: 'root-1',
      payload: {
        blocks: [
          block('producer', 0, {
            inputs: [],
            outputs: [{ name: 'total', schema: { type: 'integer' } }]
          }),
          block('consumer', 1, {
            inputs: [
              {
                name: 'total',
                schema: { type: 'integer' },
                source: {
                  block_id: 'producer',
                  output: 'total',
                  scope: 'tab'
                }
              }
            ],
            outputs: []
          })
        ]
      }
    }
  });
}
