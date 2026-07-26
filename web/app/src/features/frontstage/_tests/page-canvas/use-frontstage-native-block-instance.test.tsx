import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { useEffect, useState, type ComponentType } from 'react';
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
import { createFrontstagePageContentFixture } from '../frontstage-page-content-fixtures';

describe('PageCanvas declarative Native block lifecycle', () => {
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
      useEffect(() => () => {
        unmounts += 1;
      }, []);
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
        runtimeContext={runtimeContext('dark')}
        runtimePreparations={[initialPreparation]}
      />
    );
    await waitFor(() =>
      expect(stateful).toHaveTextContent('1:1:Changed:dark')
    );
    expect(mounts).toBe(1);
    expect(unmounts).toBe(0);

    view.rerender(
      <PageCanvas
        content={pageContent('Changed')}
        runtimeContext={runtimeContext('dark')}
        runtimePreparations={[
          preparation('source-b', 1, StatefulBlock)
        ]}
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
        runtimeContext={runtimeContext('dark')}
        runtimePreparations={[
          preparation('source-b', 1, StatefulBlock, true, {
            runtimeFingerprint: 'runtime-b'
          })
        ]}
      />
    );
    await waitFor(() => expect(mounts).toBe(3));
    expect(unmounts).toBe(2);

    view.rerender(
      <PageCanvas
        content={pageContent('Changed')}
        runtimeContext={runtimeContext('dark')}
        runtimePreparations={[
          preparation('source-b', 1, StatefulBlock, true, {
            runtimeFingerprint: 'runtime-b',
            dependencyLockIdentity: 'lock-b'
          })
        ]}
      />
    );
    await waitFor(() => expect(mounts).toBe(4));
    expect(unmounts).toBe(3);
  });

  test('D3R-AC-005/008 presents demand 0/1, unmounts 2/3, and disposes the page epoch', async () => {
    const contexts: BlockContext[] = [];
    let mounts = 0;
    let unmounts = 0;
    const LifecycleBlock = ({ ctx }: { ctx: BlockContext }) => {
      contexts.push(ctx);
      useState(() => ++mounts);
      useEffect(() => () => {
        unmounts += 1;
      }, []);
      return <div data-testid="lifecycle-native-block">ready</div>;
    };
    const view = render(
      <PageCanvas
        content={pageContent('Demand')}
        runtimePreparations={[preparation('source-a', 0, LifecycleBlock)]}
      />
    );
    const root = await nativeRoot();
    await within(root.shadow).findByTestId('lifecycle-native-block');
    const firstPublish = contexts.at(-1)!.outputs.publish;

    view.rerender(
      <PageCanvas
        content={pageContent('Demand')}
        runtimePreparations={[preparation('source-a', 1, LifecycleBlock)]}
      />
    );
    expect(mounts).toBe(1);

    for (const priority of [2, 3] as const) {
      view.rerender(
        <PageCanvas
          content={pageContent('Demand')}
          runtimePreparations={[
            preparation('source-a', priority, LifecycleBlock, false)
          ]}
        />
      );
      await waitFor(() => expect(root.shadow.childNodes).toHaveLength(0));
    }
    expect(unmounts).toBe(1);
    expect(firstPublish({})).toEqual({ ok: false, stale: true });

    view.rerender(
      <PageCanvas
        content={pageContent('Demand')}
        runtimePreparations={[preparation('source-a', 1, LifecycleBlock)]}
      />
    );
    const remountedRoot = await nativeRoot();
    await within(remountedRoot.shadow).findByTestId('lifecycle-native-block');
    expect(mounts).toBe(2);

    const secondPublish = contexts.at(-1)!.outputs.publish;
    view.unmount();
    await waitFor(() => expect(unmounts).toBe(2));
    expect(secondPublish({})).toEqual({ ok: false, stale: true });
    expect(remountedRoot.shadow.childNodes).toHaveLength(0);
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
        shouldThrow = false;
        throw new Error('controlled render failure');
      }
      return <div data-testid="recovered-native-block">recovered</div>;
    };

    try {
      render(
        <PageCanvas
          content={pageContent('Retry')}
          runtimePreparations={[
            preparation('source-a', 1, RecoveringBlock)
          ]}
        />
      );
      const firstPublish = contexts.at(-1)!.outputs.publish;
      fireEvent.click(await screen.findByRole('button', { name: /重\s*试/ }));

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
    runtimeFingerprint: string;
    dependencyLockIdentity: string;
  }> = {}
): Extract<FrontstageNativePreparationSnapshot, { status: 'ready' }> {
  const identityInput = {
    sourceSha256: sourceSha256.padEnd(64, '0'),
    runtimeFingerprint: identityOverrides.runtimeFingerprint ?? 'runtime-a',
    dependencyLockIdentity:
      identityOverrides.dependencyLockIdentity ?? 'lock-a'
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
      artifactCacheTier: 'l2'
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
