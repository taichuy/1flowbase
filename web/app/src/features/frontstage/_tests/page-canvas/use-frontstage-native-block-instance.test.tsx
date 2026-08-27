import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import {
  useEffect,
  useLayoutEffect,
  useState,
  type ComponentType
} from 'react';
import { describe, expect, test, vi } from 'vitest';

import type { BlockContext } from '@1flowbase/page-protocol';

import type { FrontstagePageContent } from '../../api/page-content';
import {
  PageCanvas,
  type FrontstagePageCanvasRuntimeContext
} from '../../components/PageCanvas';
import type {
  FrontstageNativePreparationSnapshot,
  FrontstageNativePreparedRuntime
} from '../../lib/page-canvas/native-runtime-preparation';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import { createFrontstagePageContentFixture } from '../frontstage-page-content-fixtures';

describe('PageCanvas declarative Native block lifecycle', () => {
  test('AC-013 exposes route inputs to the selected runtime block context', async () => {
    const InputBlock = ({ ctx }: { ctx: BlockContext }) => (
      <div data-testid="runtime-route-input">
        {String(ctx.inputs.record_id ?? 'missing')}
      </div>
    );

    render(
      <PageCanvas
        content={pageContent('Route input')}
        runtimeBlocks={[runtimeBlock('Route input')]}
        runtimePreparations={[preparation('source-a', 1, InputBlock)]}
        runtimeInputsByBlockId={{
          'block-1': { record_id: 'record-1' }
        }}
      />
    );

    const root = await nativeRoot();
    expect(
      await within(root.shadow).findByTestId('runtime-route-input')
    ).toHaveTextContent('record-1');
  });

  test('D3R-AC-001/004 keeps Hooks across props and theme updates, then remounts once per source/runtime/dependency identity change', async () => {
    let mounts = 0;
    let unmounts = 0;
    const StatefulBlock = ({
      props,
      ctx
    }: {
      props: Record<string, unknown>;
      ctx: BlockContext;
    }) => {
      const [mountId] = useState(() => ++mounts);
      const [localCount, setLocalCount] = useState(0);
      useEffect(
        () => () => {
          unmounts += 1;
        },
        []
      );
      return (
        <button
          data-testid="stateful-native-block"
          onClick={() => setLocalCount((current) => current + 1)}
        >
          {mountId}:{localCount}:{String(props.title)}:{ctx.theme.mode}
        </button>
      );
    };
    const initialPreparation = preparation('source-a', 1, StatefulBlock);
    const view = render(
      <PageCanvas
        content={pageContent('Initial')}
        runtimeBlocks={[runtimeBlock('Initial')]}
        runtimeContext={runtimeContext('light')}
        runtimePreparations={[initialPreparation]}
      />
    );
    const firstRoot = await nativeRoot();
    const stateful = await within(firstRoot.shadow).findByTestId(
      'stateful-native-block'
    );
    fireEvent.click(stateful);
    expect(stateful).toHaveTextContent('1:1:Initial:light');

    view.rerender(
      <PageCanvas
        content={pageContent('Changed')}
        runtimeBlocks={[runtimeBlock('Changed')]}
        runtimeContext={runtimeContext('dark')}
        runtimePreparations={[initialPreparation]}
      />
    );
    await waitFor(() => expect(stateful).toHaveTextContent('1:1:Changed:dark'));
    expect(mounts).toBe(1);
    expect(unmounts).toBe(0);

    view.rerender(
      <PageCanvas
        content={pageContent('Changed')}
        runtimeBlocks={[runtimeBlock('Changed')]}
        runtimeContext={runtimeContext('dark')}
        runtimePreparations={[preparation('source-b', 1, StatefulBlock)]}
      />
    );
    const remountedRoot = await nativeRoot();
    await waitFor(() =>
      expect(
        within(remountedRoot.shadow).getByTestId('stateful-native-block')
      ).toHaveTextContent('2:0:Changed:dark')
    );
    expect(mounts).toBe(2);
    expect(unmounts).toBe(1);

    view.rerender(
      <PageCanvas
        content={pageContent('Changed')}
        runtimeBlocks={[runtimeBlock('Changed')]}
        runtimeContext={runtimeContext('dark')}
        runtimePreparations={[
          preparation('source-b', 1, StatefulBlock, true, {
            runtimeAbi: 'runtime-b'
          })
        ]}
      />
    );
    await waitFor(() => expect(mounts).toBe(3));
    expect(unmounts).toBe(2);

    view.rerender(
      <PageCanvas
        content={pageContent('Changed')}
        runtimeBlocks={[runtimeBlock('Changed')]}
        runtimeContext={runtimeContext('dark')}
        runtimePreparations={[
          preparation('source-b', 1, StatefulBlock, true, {
            runtimeAbi: 'runtime-b',
            compilerAbi: 'compiler-b'
          })
        ]}
      />
    );
    await waitFor(() => expect(mounts).toBe(4));
    expect(unmounts).toBe(3);
  });

  test('AC-001 preserves a mounted Portal across demand changes and disposes the page epoch', async () => {
    const contexts: BlockContext[] = [];
    let mounts = 0;
    let unmounts = 0;
    const LifecycleBlock = ({ ctx }: { ctx: BlockContext }) => {
      contexts.push(ctx);
      useState(() => ++mounts);
      useEffect(
        () => () => {
          unmounts += 1;
        },
        []
      );
      return <div data-testid="lifecycle-native-block">ready</div>;
    };
    const view = render(
      <PageCanvas
        content={pageContent('Demand')}
        runtimeBlocks={[runtimeBlock('Demand')]}
        runtimePreparations={[preparation('source-a', 0, LifecycleBlock)]}
      />
    );
    const root = await nativeRoot();
    await within(root.shadow).findByTestId('lifecycle-native-block');
    const blockSlot = screen.getByTestId('block-slot-block-1');
    const measuredIntrinsicHeight = Number(
      blockSlot.getAttribute('data-flowbase-frontstage-intrinsic-height')
    );
    expect(measuredIntrinsicHeight).toBeGreaterThan(0);
    const firstPublish = contexts.at(-1)!.outputs.publish;

    view.rerender(
      <PageCanvas
        content={pageContent('Demand')}
        runtimeBlocks={[runtimeBlock('Demand')]}
        runtimePreparations={[preparation('source-a', 1, LifecycleBlock)]}
      />
    );
    expect(mounts).toBe(1);

    for (const priority of [2, 3] as const) {
      view.rerender(
        <PageCanvas
          content={pageContent('Demand')}
          runtimeBlocks={[runtimeBlock('Demand')]}
          runtimePreparations={[
            preparation('source-a', priority, LifecycleBlock)
          ]}
        />
      );
      expect(
        within(root.shadow).getByTestId('lifecycle-native-block')
      ).toHaveTextContent('ready');
      expect(blockSlot).toHaveAttribute(
        'data-flowbase-frontstage-intrinsic-height',
        String(measuredIntrinsicHeight)
      );
    }
    expect(mounts).toBe(1);
    expect(unmounts).toBe(0);
    expect(firstPublish({})).toEqual({ ok: true, stale: false });

    view.unmount();
    await waitFor(() => expect(unmounts).toBe(1));
    expect(firstPublish({})).toEqual({ ok: false, stale: true });
    expect(root.shadow.childNodes).toHaveLength(0);
  });

  test('AC-004 keeps ctx.state stable across host updates in one mount epoch', async () => {
    const StatefulContextBlock = ({ ctx }: { ctx: BlockContext }) => {
      const [revision, setRevision] = useState(0);
      return (
        <button
          data-testid="context-state-native-block"
          onClick={() => {
            ctx.patch({ count: Number(ctx.state.count ?? 0) + 1 });
            setRevision((current) => current + 1);
          }}
        >
          {String(ctx.state.count ?? 'none')}:{revision}:{ctx.theme.mode}
        </button>
      );
    };
    const ready = preparation('source-a', 1, StatefulContextBlock);
    const view = render(
      <PageCanvas
        content={pageContent('Context state')}
        runtimeBlocks={[runtimeBlock('Context state')]}
        runtimeContext={runtimeContext('light')}
        runtimePreparations={[ready]}
      />
    );
    const root = await nativeRoot();
    const button = await within(root.shadow).findByTestId(
      'context-state-native-block'
    );
    fireEvent.click(button);
    expect(button).toHaveTextContent('1:1:light');

    view.rerender(
      <PageCanvas
        content={pageContent('Context state')}
        runtimeBlocks={[runtimeBlock('Context state')]}
        runtimeContext={runtimeContext('dark')}
        runtimePreparations={[ready]}
      />
    );

    await waitFor(() => expect(button).toHaveTextContent('1:1:dark'));
  });

  test('AC-008 AC-010 exposes allocated viewport size while explicit intrinsic demand stays independent', async () => {
    let mounts = 0;
    const SizingBlock = ({ ctx }: { ctx: BlockContext }) => {
      const [localCount, setLocalCount] = useState(0);
      useState(() => ++mounts);
      const sizing = ctx.ui.sizing;
      useLayoutEffect(() => {
        sizing?.reportIntrinsicSize({ height: 320 });
      }, [sizing]);
      return (
        <button
          data-testid="sizing-native-block"
          onClick={() => setLocalCount((current) => current + 1)}
        >
          {sizing?.available.width ?? 'missing'}x
          {sizing?.available.height ?? 'missing'}:{localCount}
        </button>
      );
    };
    const ready = preparation('source-a', 1, SizingBlock);
    const view = render(
      <PageCanvas
        content={pageContent('Sizing')}
        runtimeBlocks={[runtimeBlock('Sizing')]}
        runtimeContext={runtimeContext('light')}
        runtimePreparations={[ready]}
      />
    );

    const root = await nativeRoot();
    const button = await within(root.shadow).findByTestId(
      'sizing-native-block'
    );
    await waitFor(() => expect(button).toHaveTextContent('1280x800:0'));
    await waitFor(() =>
      expect(screen.getByTestId('block-slot-block-1')).toHaveAttribute(
        'data-flowbase-frontstage-intrinsic-height',
        '320'
      )
    );
    expect(root.host.parentElement).toHaveStyle({ height: '100%' });
    expect(
      root.host.closest('[data-flowbase-frontstage-intrinsic-content]')
    ).toHaveStyle({ height: '100%' });

    fireEvent.click(button);
    view.rerender(
      <PageCanvas
        content={pageContent('Sizing')}
        runtimeBlocks={[runtimeBlock('Sizing')]}
        runtimeContext={runtimeContext('dark')}
        runtimePreparations={[ready]}
      />
    );

    await waitFor(() => expect(button).toHaveTextContent('1280x800:1'));
    expect(mounts).toBe(1);

    const PlainBlock = (_props: {
      ctx: BlockContext;
      props: Record<string, unknown>;
    }) => <div data-testid="plain-native-block">plain</div>;
    view.rerender(
      <PageCanvas
        content={pageContent('Plain')}
        runtimeBlocks={[runtimeBlock('Plain')]}
        runtimeContext={runtimeContext('dark')}
        runtimePreparations={[
          preparation('source-b', 1, PlainBlock)
        ]}
      />
    );
    await within(root.shadow).findByTestId('plain-native-block');
    expect(
      root.host.closest('[data-flowbase-frontstage-intrinsic-content]')
    ).not.toHaveStyle({ height: '100%' });
  });

  test('D3R-AC-005 render retry replaces only the failed Portal epoch', async () => {
    const contexts: BlockContext[] = [];
    let shouldThrow = true;
    const consoleError = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);
    const RecoveringBlock = ({ ctx }: { ctx: BlockContext }) => {
      contexts.push(ctx);
      if (shouldThrow) {
        throw new Error('controlled render failure');
      }
      return <div data-testid="recovered-native-block">recovered</div>;
    };

    try {
      render(
        <PageCanvas
          content={pageContent('Retry')}
          runtimeBlocks={[runtimeBlock('Retry')]}
          runtimePreparations={[preparation('source-a', 1, RecoveringBlock)]}
        />
      );
      const firstPublish = contexts.at(-1)!.outputs.publish;
      const retry = await screen.findByRole('button', { name: /重\s*试/ });
      shouldThrow = false;
      fireEvent.click(retry);

      const root = await nativeRoot();
      await within(root.shadow).findByTestId('recovered-native-block');
      expect(contexts.at(-1)!.outputs.publish).not.toBe(firstPublish);
      expect(firstPublish({})).toEqual({ ok: false, stale: true });
    } finally {
      consoleError.mockRestore();
    }
  });
});

async function nativeRoot(): Promise<{
  host: HTMLElement;
  shadow: HTMLElement;
}> {
  const selector = '[data-testid="frontstage-native-block-root-block-1"]';
  await waitFor(() => expect(document.querySelector(selector)).not.toBeNull());
  const host = document.querySelector(selector) as HTMLElement;
  await waitFor(() => expect(host.shadowRoot).not.toBeNull());
  return { host, shadow: host.shadowRoot as unknown as HTMLElement };
}

function preparation(
  sourceSha256: string,
  priority: 0 | 1 | 2 | 3,
  component: ComponentType<{
    ctx: BlockContext;
    props: Record<string, unknown>;
  }>,
  present = true,
  identityOverrides: Partial<{
    compilerAbi: string;
    runtimeAbi: string;
  }> = {}
): Extract<FrontstageNativePreparationSnapshot, { status: 'ready' }> {
  const identityInput = {
    sourceSha256: sourceSha256.padEnd(64, '0'),
    compilerAbi: identityOverrides.compilerAbi ?? 'compiler-a',
    runtimeAbi: identityOverrides.runtimeAbi ?? 'runtime-a'
  };
  return {
    status: 'ready',
    blockId: 'block-1',
    slotIndex: 0,
    priority,
    generation: 0,
    mountIntent: present
      ? { blockId: 'block-1', slotIndex: 0, identityInput }
      : null,
    prepared: {
      artifact: {} as FrontstageNativePreparedRuntime['artifact'],
      component: component as FrontstageNativePreparedRuntime['component'],
      identityInput,
      artifactCacheTier: 'l2',
      moduleAssets: []
    }
  };
}

function runtimeContext(
  mode: 'light' | 'dark'
): FrontstagePageCanvasRuntimeContext {
  return {
    currentUser: null,
    workspace: { id: 'workspace-1' },
    application: null,
    theme: { mode, tokens: {} },
    ui: { locale: 'en_US' }
  };
}

function runtimeBlock(title: string): FrontstageBlockInstance {
  return {
    id: 'block-1',
    rendererVersion: 'v1',
    sourceId: 'block-1',
    codeRef: 'block-1-code',
    sourceCodeRef: 'block-1',
    catalog: {
      providerCode: 'official',
      installationId: 'installation-1'
    },
    contribution: {
      pluginId: 'official.blocks',
      pluginVersion: '1.0.0',
      code: 'block-1'
    },
    runtime: {
      kind: 'native_react',
      entry: 'blocks/block-1.js',
      hint: 'native_react'
    },
    layout: { order: 0, region: 'main' },
    presentation: { heightMode: 'auto', height: null },
    order: 0,
    props: { title },
    ports: { inputs: [], outputs: [] }
  };
}

function pageContent(title: string): FrontstagePageContent {
  return createFrontstagePageContentFixture({
    root: {
      uid: 'root-1',
      payload: {
        blocks: [
          {
            id: 'block-1',
            renderer_version: 'v1',
            codeRef: 'block-1-code',
            catalog: {
              providerCode: 'official',
              installationId: 'installation-1'
            },
            contribution: {
              pluginId: 'official.blocks',
              pluginVersion: '1.0.0',
              code: 'block-1'
            },
            runtime: { kind: 'native_react', entry: 'blocks/block-1.js' },
            layout: { order: 0, region: 'main' },
            props: { title },
            ports: { inputs: [], outputs: [] }
          }
        ]
      }
    }
  });
}
