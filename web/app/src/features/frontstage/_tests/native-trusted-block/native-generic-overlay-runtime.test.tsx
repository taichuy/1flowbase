import { render, waitFor, within } from '@testing-library/react';
import { createRef } from 'react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import type { BlockContextSeed } from '@1flowbase/page-protocol';
import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';
import type { TooltipRef } from 'antd';

import { createFrontstageNativeReactModuleRegistry } from '../../lib/native-modules/registry';
import { FrontstageNativeTrustedBlockPortalHost } from '../../lib/native-trusted-block-react-adapter';

describe('native block generic AntD overlay host', () => {
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
    vi.restoreAllMocks();
    showPopover.mockReset();
    hidePopover.mockReset();
    Reflect.deleteProperty(HTMLElement.prototype, 'showPopover');
    Reflect.deleteProperty(HTMLElement.prototype, 'hidePopover');
  });

  test('I1931-AC-001/002 routes rc-trigger component families into one Block Top Layer', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antd = (await registry.load(
      'antd'
    )) as unknown as typeof import('antd');
    const root = document.createElement('div');
    document.body.append(root);

    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="generic-overlay:matrix"
        plan={createPlan('generic-overlay-matrix')}
        component={() => (
          <>
            <antd.Cascader
              open
              options={[{ value: 'zhejiang', label: 'Cascader Zhejiang' }]}
            />
            <antd.Select
              open
              options={[{ value: 'select', label: 'Select Alpha' }]}
            />
            <antd.TreeSelect
              open
              treeData={[{ value: 'tree', title: 'Tree Alpha' }]}
            />
            <antd.DatePicker open />
            <antd.Tooltip open title="Tooltip Alpha">
              <button type="button">Tooltip trigger</button>
            </antd.Tooltip>
            <antd.Popover open content="Popover Alpha">
              <button type="button">Popover trigger</button>
            </antd.Popover>
          </>
        )}
        ctx={createContext()}
      />
    );

    const shadowRoot = await waitFor(() => root.shadowRoot as ShadowRoot);
    const layer = await waitFor(() => {
      const candidate = shadowRoot.querySelector<HTMLElement>(
        '[data-flowbase-native-overlay-layer="generic-overlay-matrix"]'
      );
      expect(candidate).toHaveAttribute(
        'data-flowbase-native-overlay-state',
        'open'
      );
      return candidate as HTMLElement;
    });
    const overlay = within(layer);
    expect(await overlay.findByText('Cascader Zhejiang')).toBeVisible();
    expect(await overlay.findByText('Select Alpha')).toBeVisible();
    expect(await overlay.findByText('Tree Alpha')).toBeVisible();
    expect(await overlay.findByText('Tooltip Alpha')).toBeVisible();
    expect(await overlay.findByText('Popover Alpha')).toBeVisible();
    expect(layer.querySelector('[class*="-picker-dropdown"]')).not.toBeNull();
    expect(showPopover).toHaveBeenCalledOnce();
    expect(
      document.body.querySelector('[data-flowbase-native-overlay-layer]')
    ).toBeNull();
  });

  test('D1-AC-003 open Popover and Tooltip coalesce outer scroll into one Surface frame', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antd = (await registry.load(
      'antd'
    )) as unknown as typeof import('antd');
    const scrollOwner = document.createElement('div');
    scrollOwner.setAttribute('data-flowbase-frontstage-scroll-owner', '');
    const root = document.createElement('div');
    scrollOwner.append(root);
    document.body.append(scrollOwner);
    const popoverRef = createRef<TooltipRef>();
    const tooltipRef = createRef<TooltipRef>();

    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="generic-overlay:surface-anchor"
        plan={createPlan('generic-overlay-surface-anchor')}
        component={() => (
          <>
            <antd.Popover ref={popoverRef} open content="Surface Popover">
              <button type="button">Popover anchor</button>
            </antd.Popover>
            <antd.Tooltip ref={tooltipRef} defaultOpen title="Surface Tooltip">
              <button type="button">Tooltip anchor</button>
            </antd.Tooltip>
          </>
        )}
        ctx={createContext()}
      />
    );

    await waitFor(() => {
      expect(popoverRef.current).not.toBeNull();
      expect(tooltipRef.current).not.toBeNull();
    });
    await nextAnimationFrame();
    const popoverAlign = vi.fn();
    const tooltipAlign = vi.fn();
    popoverRef.current!.forceAlign = popoverAlign;
    tooltipRef.current!.forceAlign = tooltipAlign;
    const frames = installAnimationFrameQueue();

    scrollOwner.dispatchEvent(new Event('scroll'));
    scrollOwner.dispatchEvent(new Event('scroll'));
    expect(frames.callbacks).toHaveLength(1);
    frames.callbacks[0]();
    expect(popoverAlign).toHaveBeenCalledOnce();
    expect(tooltipAlign).toHaveBeenCalledOnce();
  });

  test('D1-AC-003 closed and authored-container overlays do not register Surface commits', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antd = (await registry.load(
      'antd'
    )) as unknown as typeof import('antd');
    const scrollOwner = document.createElement('div');
    scrollOwner.setAttribute('data-flowbase-frontstage-scroll-owner', '');
    const root = document.createElement('div');
    const authoredContainer = document.createElement('div');
    scrollOwner.append(root);
    document.body.append(scrollOwner, authoredContainer);
    const closedRef = createRef<TooltipRef>();
    const authoredRef = createRef<TooltipRef>();

    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="generic-overlay:surface-negative"
        plan={createPlan('generic-overlay-surface-negative')}
        component={() => (
          <>
            <antd.Tooltip ref={closedRef} open={false} title="Closed">
              <button type="button">Closed anchor</button>
            </antd.Tooltip>
            <antd.Popover
              ref={authoredRef}
              open
              content="Authored Popover"
              getPopupContainer={() => authoredContainer}
            >
              <button type="button">Authored anchor</button>
            </antd.Popover>
          </>
        )}
        ctx={createContext()}
      />
    );

    await within(authoredContainer).findByText('Authored Popover');
    await waitFor(() => {
      expect(closedRef.current).not.toBeNull();
      expect(authoredRef.current).not.toBeNull();
    });
    const closedAlign = vi.fn();
    const authoredAlign = vi.fn();
    closedRef.current!.forceAlign = closedAlign;
    authoredRef.current!.forceAlign = authoredAlign;
    const frames = installAnimationFrameQueue();

    scrollOwner.dispatchEvent(new Event('scroll'));
    window.dispatchEvent(new Event('resize'));
    frames.callbacks.forEach((callback) => callback());
    expect(closedAlign).not.toHaveBeenCalled();
    expect(authoredAlign).not.toHaveBeenCalled();
  });

  test('D1-AC-003 layout generation and dispose reject stale Surface commits', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antd = (await registry.load(
      'antd'
    )) as unknown as typeof import('antd');
    const scrollOwner = document.createElement('div');
    scrollOwner.setAttribute('data-flowbase-frontstage-scroll-owner', '');
    const root = document.createElement('div');
    scrollOwner.append(root);
    document.body.append(scrollOwner);
    const tooltipRef = createRef<TooltipRef>();
    const Block = () => (
      <antd.Tooltip ref={tooltipRef} open title="Stale Tooltip">
        <button type="button">Stale anchor</button>
      </antd.Tooltip>
    );
    const view = render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="generic-overlay:stale"
        surfaceLayoutEpoch="preview"
        plan={createPlan('generic-overlay-stale')}
        component={Block}
        ctx={createContext()}
      />
    );

    await waitFor(() => expect(tooltipRef.current).not.toBeNull());
    await nextAnimationFrame();
    const forceAlign = vi.fn();
    tooltipRef.current!.forceAlign = forceAlign;
    const frames = installAnimationFrameQueue();
    scrollOwner.dispatchEvent(new Event('scroll'));
    const staleGenerationCallback = frames.callbacks[0];

    view.rerender(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="generic-overlay:stale"
        surfaceLayoutEpoch="design"
        plan={createPlan('generic-overlay-stale')}
        component={Block}
        ctx={createContext()}
      />
    );
    staleGenerationCallback();
    expect(forceAlign).not.toHaveBeenCalled();

    scrollOwner.dispatchEvent(new Event('scroll'));
    const staleDisposeCallback = frames.callbacks.at(-1)!;
    view.unmount();
    staleDisposeCallback();
    expect(forceAlign).not.toHaveBeenCalled();
  });

  test('I1931-AC-004/006 keeps authored containers outside the default Block overlay', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antd = (await registry.load(
      'antd'
    )) as unknown as typeof import('antd');
    const root = document.createElement('div');
    const authoredContainer = document.createElement('div');
    document.body.append(root, authoredContainer);

    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="generic-overlay:authored"
        plan={createPlan('generic-overlay-authored')}
        component={() => (
          <antd.Cascader
            open
            getPopupContainer={() => authoredContainer}
            options={[{ value: 'authored', label: 'Authored option' }]}
          />
        )}
        ctx={createContext()}
      />
    );

    await within(authoredContainer).findByText('Authored option');
    const layer = root.shadowRoot?.querySelector<HTMLElement>(
      '[data-flowbase-native-overlay-layer]'
    );
    expect(layer).toHaveAttribute(
      'data-flowbase-native-overlay-state',
      'closed'
    );
    expect(showPopover).not.toHaveBeenCalled();
  });
});

function createPlan(blockId: string): NativeTrustedBlockPreparePlan {
  const source = 'export default function Block() { return null; }';
  return {
    runtime: 'native_trusted_block',
    blockId,
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

function installAnimationFrameQueue() {
  const callbacks: Array<() => void> = [];
  let sequence = 1000;
  vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    callbacks.push(() => callback(performance.now()));
    sequence += 1;
    return sequence;
  });
  vi.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => undefined);
  return { callbacks };
}

async function nextAnimationFrame(): Promise<void> {
  await new Promise<void>((resolve) => {
    window.requestAnimationFrame(() => resolve());
  });
}
