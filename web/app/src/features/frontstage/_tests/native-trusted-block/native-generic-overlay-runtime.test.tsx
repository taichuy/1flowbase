import { render, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import type { BlockContextSeed } from '@1flowbase/page-protocol';
import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';

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
