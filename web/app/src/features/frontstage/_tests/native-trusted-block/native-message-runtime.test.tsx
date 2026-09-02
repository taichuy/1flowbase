import { fireEvent, render, waitFor, within } from '@testing-library/react';
import { useState, type ReactElement } from 'react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import type { BlockContextSeed } from '@1flowbase/page-protocol';
import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';
import { message as AntdMessage } from 'antd';
import type { MessageInstance } from 'antd/es/message/interface';

import { createFrontstageNativeReactModuleRegistry } from '../../lib/native-modules/registry';
import { FrontstageNativeTrustedBlockPortalHost } from '../../lib/native-trusted-block-react-adapter';

type NativeMessageModule = {
  useMessage(): readonly [MessageInstance, ReactElement];
};

describe('native block message runtime adapter', () => {
  const showPopover = vi.fn();
  const hidePopover = vi.fn();

  beforeEach(() => {
    Object.defineProperties(HTMLElement.prototype, {
      showPopover: { configurable: true, value: showPopover },
      hidePopover: { configurable: true, value: hidePopover }
    });
  });

  afterEach(() => {
    document.body.replaceChildren();
    showPopover.mockReset();
    hidePopover.mockReset();
    Reflect.deleteProperty(HTMLElement.prototype, 'showPopover');
    Reflect.deleteProperty(HTMLElement.prototype, 'hidePopover');
  });

  test('I1922-AC-002 preserves the official static message API', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antdModule = await registry.load('antd');
    const nativeMessage = antdModule.message as typeof AntdMessage;

    expect(nativeMessage.info).toBe(AntdMessage.info);
    expect(nativeMessage.success).toBe(AntdMessage.success);
    expect(nativeMessage.error).toBe(AntdMessage.error);
    expect(nativeMessage.warning).toBe(AntdMessage.warning);
    expect(nativeMessage.loading).toBe(AntdMessage.loading);
    expect(nativeMessage.open).toBe(AntdMessage.open);
    expect(nativeMessage.destroy).toBe(AntdMessage.destroy);
  });

  test('I1922-AC-001/003 mounts useMessage notices in the Block-owned top layer', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antdModule = await registry.load('antd');
    const nativeMessage = antdModule.message as NativeMessageModule;
    const root = document.createElement('div');
    document.body.append(root);

    function Block() {
      const [api, holder] = nativeMessage.useMessage();
      return (
        <>
          {holder}
          <button type="button" onClick={() => api.info('Saved in Block')}>
            Show message
          </button>
        </>
      );
    }

    const view = render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="message:1"
        plan={createPlan()}
        component={Block}
        ctx={createContext()}
      />
    );
    const shadowRoot = await waitFor(() => root.shadowRoot as ShadowRoot);
    const queries = within(shadowRoot as unknown as HTMLElement);
    const layer = await waitFor(() => {
      const node = shadowRoot.querySelector<HTMLElement>(
        '[data-flowbase-native-message-layer]'
      );
      expect(node).not.toBeNull();
      return node as HTMLElement;
    });

    fireEvent.click(queries.getByRole('button', { name: 'Show message' }));

    expect(await within(layer).findByText('Saved in Block')).toBeVisible();
    expect(layer).toHaveAttribute('popover', 'manual');
    expect(layer).toHaveAttribute('data-flowbase-native-overlay-state', 'open');
    expect(showPopover).toHaveBeenCalledOnce();
    expect(document.body).not.toHaveTextContent('Saved in Block');

    view.unmount();
    expect(layer.isConnected).toBe(false);
  });

  test('I1922-AC-004 isolates useMessage holders across Block surfaces', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antdModule = await registry.load('antd');
    const nativeMessage = antdModule.message as NativeMessageModule;
    const firstRoot = document.createElement('div');
    const secondRoot = document.createElement('div');
    document.body.append(firstRoot, secondRoot);

    function Block({ label }: { label: string }) {
      const [api, holder] = nativeMessage.useMessage();
      const [opened, setOpened] = useState(false);
      return (
        <>
          {holder}
          <button
            type="button"
            onClick={() => {
              setOpened(true);
              api.info(label);
            }}
          >
            {opened ? `Opened ${label}` : `Open ${label}`}
          </button>
        </>
      );
    }

    render(
      <>
        <FrontstageNativeTrustedBlockPortalHost
          root={firstRoot}
          renderEpoch="message:first"
          plan={{ ...createPlan(), blockId: 'message-first' }}
          component={() => <Block label="First notice" />}
          ctx={createContext()}
        />
        <FrontstageNativeTrustedBlockPortalHost
          root={secondRoot}
          renderEpoch="message:second"
          plan={{ ...createPlan(), blockId: 'message-second' }}
          component={() => <Block label="Second notice" />}
          ctx={createContext()}
        />
      </>
    );
    const firstShadow = await waitFor(() => firstRoot.shadowRoot as ShadowRoot);
    const secondShadow = await waitFor(
      () => secondRoot.shadowRoot as ShadowRoot
    );
    fireEvent.click(
      within(firstShadow as unknown as HTMLElement).getByRole('button', {
        name: 'Open First notice'
      })
    );

    const firstLayer = firstShadow.querySelector<HTMLElement>(
      '[data-flowbase-native-message-layer]'
    ) as HTMLElement;
    const secondLayer = secondShadow.querySelector<HTMLElement>(
      '[data-flowbase-native-message-layer]'
    ) as HTMLElement;
    expect(await within(firstLayer).findByText('First notice')).toBeVisible();
    expect(secondLayer).not.toHaveTextContent('First notice');
    expect(secondLayer).toHaveAttribute(
      'data-flowbase-native-overlay-state',
      'closed'
    );
  });
});

function createPlan(): NativeTrustedBlockPreparePlan {
  const source = 'export default function Block() { return null; }';
  return {
    runtime: 'native_trusted_block',
    blockId: 'native-message-block',
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
