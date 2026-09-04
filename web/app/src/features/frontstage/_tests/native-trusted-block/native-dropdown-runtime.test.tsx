import { fireEvent, render, waitFor, within } from '@testing-library/react';
import { createRef, useEffect, type ComponentType } from 'react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import type { BlockContextSeed } from '@1flowbase/page-protocol';
import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';
import type { DropdownProps } from 'antd';

import { createFrontstageNativeReactModuleRegistry } from '../../lib/native-modules/registry';
import { FrontstageNativeTrustedBlockPortalHost } from '../../lib/native-trusted-block-react-adapter';

describe('native block Dropdown runtime adapter', () => {
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

  test('I1915-AC-001/003/005 opens inside a Block-scoped top-layer surface', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antdModule = await registry.load('antd');
    const Dropdown = antdModule.Dropdown as ComponentType<DropdownProps>;
    const root = document.createElement('div');
    document.body.append(root);
    const view = render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="dropdown:1"
        plan={createPlan()}
        component={() => (
          <Dropdown
            trigger={['click']}
            menu={{ items: [{ key: 'profile', label: 'Profile' }] }}
          >
            <button type="button">Open menu</button>
          </Dropdown>
        )}
        ctx={createContext()}
      />
    );
    const shadowRoot = await waitFor(() => {
      expect(root.shadowRoot).not.toBeNull();
      return root.shadowRoot as ShadowRoot;
    });

    fireEvent.click(
      within(shadowRoot as unknown as HTMLElement).getByRole('button', {
        name: 'Open menu'
      })
    );

    const layer = await waitFor(() => {
      const node = shadowRoot.querySelector<HTMLElement>(
        '[data-flowbase-native-overlay-layer="native-dropdown-block"]'
      );
      expect(node).not.toBeNull();
      return node as HTMLElement;
    });
    expect(layer).toHaveAttribute('popover', 'manual');
    expect(layer).toHaveAttribute('data-flowbase-native-overlay-state', 'open');
    expect(showPopover).toHaveBeenCalledOnce();
    expect(within(layer).getByText('Profile')).toBeVisible();
    expect(
      document.body.querySelector('[data-flowbase-native-overlay-layer]')
    ).toBeNull();

    view.unmount();
    expect(layer.isConnected).toBe(false);
  });

  test('I1915-AC-002/004 resets only the overlay generation when the layout epoch changes', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antdModule = await registry.load('antd');
    const Dropdown = antdModule.Dropdown as ComponentType<DropdownProps>;
    const root = document.createElement('div');
    document.body.append(root);
    const mounted = vi.fn();
    const unmounted = vi.fn();
    const Block = () => {
      useEffect(() => {
        mounted();
        return unmounted;
      }, []);
      return (
        <Dropdown
          trigger={['click']}
          menu={{ items: [{ key: 'profile', label: 'Profile' }] }}
        >
          <button type="button">Open menu</button>
        </Dropdown>
      );
    };
    const view = render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="dropdown:stable"
        surfaceLayoutEpoch="preview"
        plan={createPlan()}
        component={Block}
        ctx={createContext()}
      />
    );
    const shadowRoot = await waitFor(() => root.shadowRoot as ShadowRoot);
    const queries = within(shadowRoot as unknown as HTMLElement);
    const firstTrigger = queries.getByRole('button', { name: 'Open menu' });
    fireEvent.click(firstTrigger);
    await queries.findByText('Profile');
    expect(showPopover).toHaveBeenCalledTimes(1);

    view.rerender(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="dropdown:stable"
        surfaceLayoutEpoch="design"
        plan={createPlan()}
        component={Block}
        ctx={createContext()}
      />
    );

    await waitFor(() => expect(hidePopover).toHaveBeenCalledTimes(1));
    const secondTrigger = queries.getByRole('button', { name: 'Open menu' });
    expect(secondTrigger).not.toBe(firstTrigger);
    expect(mounted).toHaveBeenCalledTimes(1);
    expect(unmounted).not.toHaveBeenCalled();

    fireEvent.click(secondTrigger);
    await waitFor(() => expect(showPopover).toHaveBeenCalledTimes(2));
    expect(queries.getByText('Profile')).toBeVisible();
  });

  test('I1915-AC-006 preserves one open transition for the hover intent and menu close', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antdModule = await registry.load('antd');
    const Dropdown = antdModule.Dropdown as ComponentType<DropdownProps>;
    const root = document.createElement('div');
    document.body.append(root);
    const onOpenChange = vi.fn();
    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="dropdown:hover"
        plan={createPlan()}
        component={() => (
          <Dropdown
            menu={{ items: [{ key: 'profile', label: 'Profile' }] }}
            onOpenChange={onOpenChange}
          >
            <button type="button">Hover menu</button>
          </Dropdown>
        )}
        ctx={createContext()}
      />
    );
    const shadowRoot = await waitFor(() => root.shadowRoot as ShadowRoot);
    const queries = within(shadowRoot as unknown as HTMLElement);
    const trigger = queries.getByRole('button', { name: 'Hover menu' });

    fireEvent.pointerOver(trigger);
    const menuItem = await queries.findByText('Profile');
    expect(onOpenChange).toHaveBeenCalledTimes(1);
    expect(onOpenChange).toHaveBeenLastCalledWith(true, {
      source: 'trigger'
    });

    fireEvent.click(menuItem);
    await waitFor(() =>
      expect(onOpenChange).toHaveBeenLastCalledWith(false, { source: 'menu' })
    );
  });

  test('I1924-AC-002/004 keeps a cascading submenu inside the Block top-layer surface', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antdModule = await registry.load('antd');
    const Dropdown = antdModule.Dropdown as ComponentType<DropdownProps>;
    const root = document.createElement('div');
    document.body.append(root);
    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="dropdown:cascading"
        plan={createPlan()}
        component={() => (
          <Dropdown
            menu={{
              subMenuOpenDelay: 0,
              items: [
                {
                  key: 'sub',
                  label: 'sub menu',
                  children: [{ key: 'child', label: 'child menu item' }]
                }
              ]
            }}
          >
            <button type="button">Cascading menu</button>
          </Dropdown>
        )}
        ctx={createContext()}
      />
    );
    const shadowRoot = await waitFor(() => root.shadowRoot as ShadowRoot);
    const queries = within(shadowRoot as unknown as HTMLElement);

    fireEvent.pointerOver(
      queries.getByRole('button', { name: 'Cascading menu' })
    );
    const submenuTitle = await queries.findByText('sub menu');
    fireEvent.mouseEnter(
      submenuTitle.closest('[role="menuitem"]') as HTMLElement
    );

    const child = await queries.findByText('child menu item');
    const layer = shadowRoot.querySelector<HTMLElement>(
      '[data-flowbase-native-overlay-layer]'
    );
    expect(child.closest('[data-flowbase-native-overlay-layer]')).toBe(layer);

    fireEvent.click(child);
    await waitFor(() =>
      expect(layer).toHaveAttribute(
        'data-flowbase-native-overlay-state',
        'closed'
      )
    );
  });

  test('I1915-AC-006 keeps a controlled open Dropdown in one Top Layer across a layout epoch change', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antdModule = await registry.load('antd');
    const Dropdown = antdModule.Dropdown as ComponentType<DropdownProps>;
    const root = document.createElement('div');
    document.body.append(root);
    const Block = () => (
      <Dropdown open menu={{ items: [{ key: 'profile', label: 'Profile' }] }}>
        <button type="button">Controlled menu</button>
      </Dropdown>
    );
    const view = render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="dropdown:controlled"
        surfaceLayoutEpoch="preview"
        plan={createPlan()}
        component={Block}
        ctx={createContext()}
      />
    );
    const shadowRoot = await waitFor(() => root.shadowRoot as ShadowRoot);
    await within(shadowRoot as unknown as HTMLElement).findByText('Profile');
    const layer = shadowRoot.querySelector<HTMLElement>(
      '[data-flowbase-native-overlay-layer]'
    );
    expect(layer).toHaveAttribute('data-flowbase-native-overlay-state', 'open');

    view.rerender(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="dropdown:controlled"
        surfaceLayoutEpoch="design"
        plan={createPlan()}
        component={Block}
        ctx={createContext()}
      />
    );

    await waitFor(() =>
      expect(layer).toHaveAttribute(
        'data-flowbase-native-overlay-state',
        'open'
      )
    );
    expect(hidePopover).not.toHaveBeenCalled();
    expect(showPopover).toHaveBeenCalledOnce();
  });

  test('I1923-AC-001/002 keeps a fixed virtual trigger in viewport coordinates', async () => {
    let containingBlockLeft = 200;
    let containingBlockTop = 300;
    const originalGetBoundingClientRect =
      HTMLElement.prototype.getBoundingClientRect;
    const rectSpy = vi
      .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
      .mockImplementation(function (this: HTMLElement) {
        if (!this.hasAttribute('data-testid')) {
          return originalGetBoundingClientRect.call(this);
        }
        const left = Number.parseFloat(this.style.left) + containingBlockLeft;
        const top = Number.parseFloat(this.style.top) + containingBlockTop;
        return {
          x: left,
          y: top,
          width: 1,
          height: 1,
          top,
          right: left + 1,
          bottom: top + 1,
          left,
          toJSON: () => ({})
        };
      });

    try {
      const registry = createFrontstageNativeReactModuleRegistry();
      const antdModule = await registry.load('antd');
      const Dropdown = antdModule.Dropdown as ComponentType<DropdownProps>;
      const root = document.createElement('div');
      document.body.append(root);
      render(
        <FrontstageNativeTrustedBlockPortalHost
          root={root}
          renderEpoch="dropdown:virtual-anchor"
          plan={createPlan()}
          component={() => (
            <Dropdown
              open
              trigger={[]}
              menu={{ items: [{ key: 'mark', label: 'Mark keyword' }] }}
            >
              <span
                aria-hidden
                data-testid="virtual-anchor"
                style={{
                  position: 'fixed',
                  left: 120,
                  top: 80,
                  width: 1,
                  height: 1,
                  pointerEvents: 'none'
                }}
              />
            </Dropdown>
          )}
          ctx={createContext()}
        />
      );
      const shadowRoot = await waitFor(() => root.shadowRoot as ShadowRoot);
      const anchor = within(shadowRoot as unknown as HTMLElement).getByTestId(
        'virtual-anchor'
      );

      await waitFor(() => {
        expect(anchor.getBoundingClientRect()).toMatchObject({
          left: 120,
          top: 80
        });
      });
      expect(
        within(shadowRoot as unknown as HTMLElement).getByText('Mark keyword')
      ).toBeVisible();

      containingBlockLeft = 150;
      containingBlockTop = 250;
      window.dispatchEvent(new Event('scroll'));

      await waitFor(() => {
        expect(anchor.getBoundingClientRect()).toMatchObject({
          left: 120,
          top: 80
        });
      });
    } finally {
      rectSpy.mockRestore();
    }
  });

  test('D1-AC-004 controlled negative exposes no public Dropdown realign ref', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antdModule = await registry.load('antd');
    const Dropdown = antdModule.Dropdown as (typeof import('antd'))['Dropdown'];
    const dropdownRef = createRef<HTMLElement>();
    const root = document.createElement('div');
    document.body.append(root);

    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="dropdown:public-ref"
        plan={createPlan()}
        component={() => (
          <Dropdown
            ref={dropdownRef}
            open
            align={{ offset: [0, 4] }}
            menu={{ items: [{ key: 'profile', label: 'Profile' }] }}
          >
            <button type="button">Public ref menu</button>
          </Dropdown>
        )}
        ctx={createContext()}
      />
    );

    const trigger = await within(
      (await waitFor(() => root.shadowRoot)) as unknown as HTMLElement
    ).findByRole('button', { name: 'Public ref menu' });
    expect(dropdownRef.current).toBe(trigger);
    expect(dropdownRef.current).not.toHaveProperty('forceAlign');
  });
});

function createPlan(): NativeTrustedBlockPreparePlan {
  const source = 'export default function Block() { return null; }';
  return {
    runtime: 'native_trusted_block',
    blockId: 'native-dropdown-block',
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
