import { fireEvent, render, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import type { BlockContextSeed } from '@1flowbase/page-protocol';
import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';

import { createFrontstageNativeReactModuleRegistry } from '../../lib/native-modules/registry';
import { FrontstageNativeTrustedBlockPortalHost } from '../../lib/native-trusted-block-react-adapter';

describe('native block notification effect scope', () => {
  const showPopover = vi.fn();
  const hidePopover = vi.fn();

  beforeEach(() => {
    Object.defineProperties(HTMLElement.prototype, {
      showPopover: { configurable: true, value: showPopover },
      hidePopover: { configurable: true, value: hidePopover }
    });
  });

  test('AC-002 rejects static notification effects at the injected module boundary', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antd = (await registry.load(
      'antd'
    )) as unknown as typeof import('antd');

    expect(() =>
      antd.notification.open({ title: 'Ambient notification' })
    ).toThrow(/static notification method 'open'.*App\.useApp/u);
    expect(document.body).not.toHaveTextContent('Ambient notification');
  });

  afterEach(() => {
    document.body.replaceChildren();
    showPopover.mockReset();
    hidePopover.mockReset();
    Reflect.deleteProperty(HTMLElement.prototype, 'showPopover');
    Reflect.deleteProperty(HTMLElement.prototype, 'hidePopover');
  });

  test('AC-002/003 owns App.useApp notifications and invalidates them with the layout epoch', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antd = (await registry.load(
      'antd'
    )) as unknown as typeof import('antd');
    const root = document.createElement('div');
    document.body.append(root);

    function Block() {
      const { notification } = antd.App.useApp();
      return (
        <button
          type="button"
          onClick={() =>
            notification.open({
              title: 'Block-owned notification',
              duration: 0
            })
          }
        >
          Open notification
        </button>
      );
    }

    const plan = createPlan();
    const view = render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="notification:1"
        surfaceLayoutEpoch="preview"
        plan={plan}
        component={Block}
        ctx={createContext()}
      />
    );
    const shadowRoot = await waitFor(() => root.shadowRoot as ShadowRoot);
    fireEvent.click(
      within(shadowRoot as unknown as HTMLElement).getByRole('button', {
        name: 'Open notification'
      })
    );
    const layer = shadowRoot.querySelector<HTMLElement>(
      `[data-flowbase-native-overlay-layer="${plan.blockId}"]`
    ) as HTMLElement;
    expect(
      await within(layer).findByText('Block-owned notification')
    ).toBeVisible();
    expect(layer.getRootNode()).toBe(shadowRoot);
    expect(document.body).not.toHaveTextContent('Block-owned notification');

    view.rerender(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="notification:1"
        surfaceLayoutEpoch="design"
        plan={plan}
        component={Block}
        ctx={createContext()}
      />
    );

    await waitFor(() =>
      expect(layer).not.toHaveTextContent('Block-owned notification')
    );
    expect(layer).toHaveAttribute(
      'data-flowbase-native-overlay-state',
      'closed'
    );
  });

  test('AC-002/003 owns notification.useNotification resources', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antd = (await registry.load(
      'antd'
    )) as unknown as typeof import('antd');
    const root = document.createElement('div');
    document.body.append(root);

    function Block() {
      const [notificationApi, holder] = antd.notification.useNotification();
      return (
        <>
          {holder}
          <button
            type="button"
            onClick={() =>
              notificationApi.open({
                title: 'Hook notification',
                duration: 0
              })
            }
          >
            Open Hook notification
          </button>
        </>
      );
    }

    const plan = createPlan();
    const view = render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="notification-hook:1"
        surfaceLayoutEpoch="preview"
        plan={plan}
        component={Block}
        ctx={createContext()}
      />
    );
    const shadowRoot = await waitFor(() => root.shadowRoot as ShadowRoot);
    fireEvent.click(
      within(shadowRoot as unknown as HTMLElement).getByRole('button', {
        name: 'Open Hook notification'
      })
    );
    const layer = shadowRoot.querySelector<HTMLElement>(
      `[data-flowbase-native-overlay-layer="${plan.blockId}"]`
    ) as HTMLElement;
    expect(await within(layer).findByText('Hook notification')).toBeVisible();

    view.rerender(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="notification-hook:1"
        surfaceLayoutEpoch="design"
        plan={plan}
        component={Block}
        ctx={createContext()}
      />
    );

    await waitFor(() =>
      expect(layer).not.toHaveTextContent('Hook notification')
    );
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
