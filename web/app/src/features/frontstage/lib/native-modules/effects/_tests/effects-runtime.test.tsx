import { StyleContext } from '@ant-design/cssinjs';
import { ConfigProvider, theme } from 'antd';
import { fireEvent, render, waitFor, within } from '@testing-library/react';
import { useContext } from 'react';
import { afterEach, describe, expect, test, vi } from 'vitest';

import type { BlockContextSeed } from '@1flowbase/page-protocol';
import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';
import { FrontstageNativeTrustedBlockPortalHost } from '../../../native-trusted-block-react-adapter';
import { createFrontstageNativeReactModuleRegistry } from '../../registry';
import { StyleProvider } from '../cssinjs-runtime';
import { HappyProvider } from '../happy-work-runtime';

function WaveButton() {
  const { wave } = useContext(ConfigProvider.ConfigContext);
  const { token, hashId } = theme.useToken();
  return (
    <button
      onClick={(event) =>
        wave?.showEffect?.(event.currentTarget, {
          token,
          hashId,
          event: event.nativeEvent,
          className: hashId,
          component: 'Button'
        })
      }
    >
      Wave
    </button>
  );
}

function mountBlock(Component: () => React.ReactNode) {
  const root = document.createElement('div');
  document.body.append(root);
  const plan = createPlan();
  const onRuntimeError = vi.fn();
  const renderBlock = (epoch: string) => (
    <FrontstageNativeTrustedBlockPortalHost
      onRuntimeError={onRuntimeError}
      root={root}
      renderEpoch="effects:1"
      surfaceLayoutEpoch={epoch}
      plan={plan}
      component={Component}
      ctx={createContext()}
    />
  );
  const view = render(renderBlock('preview'));
  return { root, view, renderBlock, onRuntimeError };
}

afterEach(() => {
  document.body.replaceChildren();
});

describe('Block style and Happy Work modules', () => {
  test('AC-001 registers only the supported runtime exports', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    expect(Object.keys(await registry.load('@ant-design/cssinjs'))).toEqual([
      'StyleProvider'
    ]);
    expect(
      Object.keys(await registry.load('@ant-design/happy-work-theme'))
    ).toEqual(['HappyProvider']);
  });

  test('AC-003 nested StyleProvider inherits owner cache and root while changing priority', async () => {
    let outer: React.ContextType<typeof StyleContext>;
    let inner: React.ContextType<typeof StyleContext>;
    function Probe() {
      inner = useContext(StyleContext);
      return <span>Ready</span>;
    }
    function Block() {
      outer = useContext(StyleContext);
      return (
        <StyleProvider hashPriority="high">
          <Probe />
        </StyleProvider>
      );
    }
    const { root, view } = mountBlock(Block);
    await waitFor(() => expect(inner.hashPriority).toBe('high'));
    expect(inner!.container).toBe(root.shadowRoot);
    expect(inner!.cache).toBe(outer!.cache);
    expect(outer!.hashPriority).toBe('low');
    view.unmount();
  });

  test('AC-003 rejects replacing the Block style container', async () => {
    const errorLog = vi.spyOn(console, 'error').mockImplementation(() => {});
    try {
      function EscapedBlock() {
        return (
          <StyleProvider container={document.body}>
            <span>Escape</span>
          </StyleProvider>
        );
      }
      const fixture = mountBlock(EscapedBlock);
      await waitFor(() => expect(fixture.onRuntimeError).toHaveBeenCalled());
      expect(fixture.onRuntimeError.mock.calls[0][0].message).toMatch(
        /inherit its container and cache/
      );
      fixture.view.unmount();
    } finally {
      errorLog.mockRestore();
    }
  });

  test('AC-002/004 renders upstream particles only in the owning Block and clears on layout change', async () => {
    function Block() {
      return (
        <HappyProvider>
          <WaveButton />
        </HappyProvider>
      );
    }
    const first = mountBlock(Block);
    const second = mountBlock(Block);
    const shadow = await waitFor(() => {
      expect(first.root.shadowRoot).not.toBeNull();
      return first.root.shadowRoot!;
    });
    const button = within(shadow as unknown as HTMLElement).getByRole('button');
    fireEvent.click(button);
    await waitFor(() =>
      expect(shadow.querySelector('.happy-wave-dot')).not.toBeNull()
    );
    expect(document.body.querySelector('.happy-wave')).toBeNull();
    expect(second.root.shadowRoot!.querySelector('.happy-wave')).toBeNull();
    expect(
      shadow.querySelector('[data-flowbase-happy-effect]')!.getRootNode()
    ).toBe(shadow);
    expect(
      [...shadow.querySelectorAll('style')].some((s) =>
        s.textContent?.includes('happy-wave')
      )
    ).toBe(true);
    fireEvent.click(button);
    expect(
      shadow.querySelectorAll('[data-flowbase-happy-effect]')
    ).toHaveLength(2);
    first.view.rerender(first.renderBlock('design'));
    await waitFor(() =>
      expect(shadow.querySelector('[data-flowbase-happy-effect]')).toBeNull()
    );
    expect(
      button
        .getAttributeNames()
        .some((name) => name.startsWith('data-happy-wave-target'))
    ).toBe(false);
    first.view.unmount();
    second.view.unmount();
  });

  test('AC-002 cleans the target when unmounted during an active upstream animation', async () => {
    function Block() {
      return (
        <HappyProvider>
          <WaveButton />
        </HappyProvider>
      );
    }
    const { root, view } = mountBlock(Block);
    await waitFor(() => expect(root.shadowRoot).not.toBeNull());
    const shadow = root.shadowRoot!;
    const button = within(shadow as unknown as HTMLElement).getByRole('button');
    fireEvent.click(button);
    await waitFor(() =>
      expect(
        button
          .getAttributeNames()
          .some((name) => name.startsWith('data-happy-wave-target'))
      ).toBe(true)
    );
    view.unmount();
    expect(
      button
        .getAttributeNames()
        .some((name) => name.startsWith('data-happy-wave-target'))
    ).toBe(false);
    expect(shadow.querySelector('[data-flowbase-happy-effect]')).toBeNull();
  });

  test('AC-002 removes completed effects and respects disabled', async () => {
    let disabled = false;
    function Block() {
      return (
        <HappyProvider disabled={disabled}>
          <WaveButton />
        </HappyProvider>
      );
    }
    const fixture = mountBlock(Block);
    await waitFor(() => expect(fixture.root.shadowRoot).not.toBeNull());
    const shadow = fixture.root.shadowRoot!;
    const button = within(shadow as unknown as HTMLElement).getByRole('button');
    fireEvent.click(button);
    expect(shadow.querySelector('[data-flowbase-happy-effect]')).not.toBeNull();
    await waitFor(() =>
      expect(shadow.querySelector('[data-flowbase-happy-effect]')).toBeNull()
    );
    disabled = true;
    fixture.view.rerender(fixture.renderBlock('preview'));
    fireEvent.click(button);
    expect(shadow.querySelector('[data-flowbase-happy-effect]')).toBeNull();
    fixture.view.unmount();
  });
});

function createPlan(): NativeTrustedBlockPreparePlan {
  const source = 'export default function Block() { return null; }';
  return {
    runtime: 'native_trusted_block',
    blockId: 'native-notification-block',
    entry: 'default',
    source,
    normalizedSource: source,
    props: {},
    requiredPermissions: ['ui_block.javascript.native']
  };
}

function createContext(): BlockContextSeed {
  return {
    currentUser: null,
    workspace: { id: 'workspace-1' },
    application: null,
    page: { id: 'page-1', route: '/page-1' },
    inputs: {},
    outputs: { publish: vi.fn() },
    params: {},
    props: {},
    state: {},
    patch: vi.fn(),
    api: {
      get: vi.fn(),
      post: vi.fn(),
      put: vi.fn(),
      patch: vi.fn(),
      delete: vi.fn(),
      head: vi.fn(),
      options: vi.fn(),
      stream: vi.fn()
    },
    events: { emit: vi.fn() },
    navigation: { openBlock: vi.fn() },
    theme: { mode: 'light', tokens: {} },
    ui: {}
  };
}
