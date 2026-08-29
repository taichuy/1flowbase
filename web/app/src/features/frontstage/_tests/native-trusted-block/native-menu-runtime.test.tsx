import { fireEvent, render, waitFor, within } from '@testing-library/react';
import { createRef, type ComponentType, type Ref } from 'react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import type { BlockContextSeed } from '@1flowbase/page-protocol';
import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';
import type { MenuProps, MenuRef } from 'antd';

import { createFrontstageNativeReactModuleRegistry } from '../../lib/native-modules/registry';
import { FrontstageNativeTrustedBlockPortalHost } from '../../lib/native-trusted-block-react-adapter';

describe('native block Menu runtime adapter', () => {
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

  test('I1928-AC-001/004 renders a controlled horizontal popup in the Block top layer', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antdModule = await registry.load('antd');
    const Menu = antdModule.Menu as ComponentType<
      MenuProps & { ref?: Ref<MenuRef> }
    > & {
      Item: unknown;
      SubMenu: unknown;
      ItemGroup: unknown;
      Divider: unknown;
    };
    const root = document.createElement('div');
    const menuRef = createRef<MenuRef>();
    document.body.append(root);

    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="menu:controlled"
        plan={createPlan()}
        component={() => (
          <Menu
            ref={menuRef}
            mode="horizontal"
            openKeys={['features']}
            items={[
              {
                key: 'features',
                label: 'Features',
                children: [{ key: 'components', label: 'Components' }]
              }
            ]}
            popupRender={() => <div>Custom popup panel</div>}
          />
        )}
        ctx={createContext()}
      />
    );

    const shadowRoot = await waitFor(() => root.shadowRoot as ShadowRoot);
    const layer = await waitFor(() => {
      const node = shadowRoot.querySelector<HTMLElement>(
        '[data-flowbase-native-overlay-layer="native-menu-block"]'
      );
      expect(node).not.toBeNull();
      return node as HTMLElement;
    });
    expect(layer).toHaveAttribute('popover', 'manual');
    await waitFor(() =>
      expect(layer).toHaveAttribute(
        'data-flowbase-native-overlay-state',
        'open'
      )
    );
    expect(showPopover).toHaveBeenCalledOnce();
    await waitFor(() =>
      expect(within(layer).getByText('Custom popup panel')).toBeVisible()
    );
    expect(menuRef.current).not.toBeNull();
    expect(Menu.Item).toBeDefined();
    expect(Menu.SubMenu).toBeDefined();
    expect(Menu.ItemGroup).toBeDefined();
    expect(Menu.Divider).toBeDefined();
  });

  test('I1928-AC-002 preserves an authored popup container', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antdModule = await registry.load('antd');
    const Menu = antdModule.Menu as ComponentType<MenuProps>;
    const root = document.createElement('div');
    const authoredContainer = document.createElement('div');
    document.body.append(root, authoredContainer);

    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="menu:authored-container"
        plan={createPlan()}
        component={() => (
          <Menu
            getPopupContainer={() => authoredContainer}
            mode="horizontal"
            openKeys={['resources']}
            items={[
              {
                key: 'resources',
                label: 'Resources',
                children: [{ key: 'docs', label: 'Docs' }]
              }
            ]}
          />
        )}
        ctx={createContext()}
      />
    );

    const shadowRoot = await waitFor(() => root.shadowRoot as ShadowRoot);
    await waitFor(() =>
      expect(within(authoredContainer).getByText('Docs')).toBeVisible()
    );
    expect(
      shadowRoot.querySelector('[data-flowbase-native-overlay-layer]')
    ).toHaveAttribute('data-flowbase-native-overlay-state', 'closed');
    expect(showPopover).not.toHaveBeenCalled();
  });

  test('I1928-AC-003 closes an uncontrolled popup after the layout epoch changes', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antdModule = await registry.load('antd');
    const Menu = antdModule.Menu as ComponentType<MenuProps>;
    const root = document.createElement('div');
    document.body.append(root);
    const Block = () => (
      <Menu
        mode="horizontal"
        subMenuOpenDelay={0}
        items={[
          {
            key: 'features',
            label: 'Features',
            children: [{ key: 'components', label: 'Components' }]
          }
        ]}
      />
    );
    const view = render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="menu:stable"
        surfaceLayoutEpoch="preview"
        plan={createPlan()}
        component={Block}
        ctx={createContext()}
      />
    );
    const shadowRoot = await waitFor(() => root.shadowRoot as ShadowRoot);
    fireEvent.mouseEnter(
      within(shadowRoot as unknown as HTMLElement).getByText('Features')
    );
    const layer = await waitFor(() => {
      const node = shadowRoot.querySelector<HTMLElement>(
        '[data-flowbase-native-overlay-layer]'
      );
      expect(node).toHaveAttribute(
        'data-flowbase-native-overlay-state',
        'open'
      );
      return node as HTMLElement;
    });

    view.rerender(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="menu:stable"
        surfaceLayoutEpoch="design"
        plan={createPlan()}
        component={Block}
        ctx={createContext()}
      />
    );

    await waitFor(() =>
      expect(layer).toHaveAttribute(
        'data-flowbase-native-overlay-state',
        'closed'
      )
    );
    expect(hidePopover).toHaveBeenCalled();
  });
});

function createPlan(): NativeTrustedBlockPreparePlan {
  const source = 'export default function Block() { return null; }';
  return {
    runtime: 'native_trusted_block',
    blockId: 'native-menu-block',
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
