import { fireEvent, render, waitFor, within } from '@testing-library/react';
import type { ComponentType } from 'react';
import { afterEach, describe, expect, test, vi } from 'vitest';

import type { BlockContextSeed } from '@1flowbase/page-protocol';
import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';
import type { AffixProps } from 'antd';

import { createFrontstageNativeReactModuleRegistry } from '../../lib/native-modules/registry';
import { FrontstageNativeTrustedBlockPortalHost } from '../../lib/native-trusted-block-react-adapter';

describe('native block Affix runtime adapter', () => {
  afterEach(() => {
    document.body.replaceChildren();
  });

  test('I1914-AC-001/002/003 pins against the surface owner and releases at the Block boundary', async () => {
    const onChange = vi.fn();
    const fixture = await mountAffixSurface(onChange);
    const layer = await findAffixLayer(fixture);
    const placeholder = fixture.shadowRoot.querySelector<HTMLElement>(
      '[data-flowbase-native-affix-placeholder]'
    );
    const sentinel = fixture.shadowRoot.querySelector<HTMLElement>(
      '[data-flowbase-native-affix-sentinel]'
    );
    const mount = layer.shadowRoot?.querySelector<HTMLElement>(
      '[data-flowbase-native-affix-mount]'
    );
    if (!placeholder || !sentinel || !mount) {
      throw new Error('Missing Native Affix geometry nodes.');
    }

    fixture.scrollOwner.getBoundingClientRect = () =>
      domRect({ top: 100, left: 0, width: 800, height: 500 });
    fixture.root.getBoundingClientRect = () =>
      domRect({
        top: 300 - fixture.scrollOwner.scrollTop,
        left: 40,
        width: 600,
        height: 1_000
      });
    placeholder.getBoundingClientRect = () =>
      domRect({
        top: 340 - fixture.scrollOwner.scrollTop,
        left: 60,
        width: 300,
        height: 80
      });
    sentinel.getBoundingClientRect = placeholder.getBoundingClientRect;
    mount.getBoundingClientRect = () =>
      domRect({ top: 0, left: 0, width: 300, height: 80 });

    fixture.scrollOwner.scrollTop = 0;
    fireEvent.scroll(fixture.scrollOwner);
    await nextAnimationFrame();
    expect(layer).toHaveAttribute('data-flowbase-native-affix-state', 'flow');

    fixture.scrollOwner.scrollTop = 260;
    fireEvent.scroll(fixture.scrollOwner);
    await nextAnimationFrame();
    expect(layer).toHaveAttribute('data-flowbase-native-affix-state', 'pinned');
    expect(mount).toHaveStyle({ position: 'sticky', top: '24px' });
    expect(layer).toHaveStyle({
      height: '1000px',
      left: '60px',
      top: '200px',
      width: '300px'
    });
    expect(onChange).toHaveBeenLastCalledWith(true);

    fixture.scrollOwner.scrollTop = 262;
    fireEvent.scroll(fixture.scrollOwner);
    await nextAnimationFrame();
    expect(onChange).toHaveBeenCalledTimes(1);

    fixture.scrollOwner.scrollTop = 1_150;
    fireEvent.scroll(fixture.scrollOwner);
    await nextAnimationFrame();
    expect(layer).toHaveAttribute(
      'data-flowbase-native-affix-state',
      'boundary'
    );
    expect(onChange).toHaveBeenLastCalledWith(false);
    expect(onChange).toHaveBeenCalledTimes(2);
  });

  test('I1914-AC-004/005 keeps the portal scoped and disposes its owner lease', async () => {
    const fixture = await mountAffixSurface();
    const layer = await findAffixLayer(fixture);

    expect(
      within(layer.shadowRoot as unknown as HTMLElement).getByText(
        'Fixed toolbar'
      )
    ).toBeVisible();
    expect(
      fixture.shadowRoot.querySelector('[data-flowbase-native-affix-layer]')
    ).toBeNull();
    expect(fixture.scrollOwner).toHaveStyle({ position: 'relative' });

    fixture.view.unmount();

    expect(layer.isConnected).toBe(false);
    expect(fixture.scrollOwner).toHaveStyle({ position: 'static' });
  });

  test('I1914-AC-005 isolates two Affix portals and releases their shared owner deterministically', async () => {
    const scrollOwner = document.createElement('div');
    scrollOwner.style.overflowY = 'auto';
    scrollOwner.style.position = 'static';
    document.body.append(scrollOwner);
    const first = await mountAffixSurface(vi.fn(), 'first', scrollOwner);
    const second = await mountAffixSurface(vi.fn(), 'second', scrollOwner);
    const firstLayer = await findAffixLayer(first);
    const secondLayer = await findAffixLayer(second);

    expect(
      within(firstLayer.shadowRoot as unknown as HTMLElement).getByText(
        'Fixed toolbar first'
      )
    ).toBeVisible();
    expect(
      within(secondLayer.shadowRoot as unknown as HTMLElement).getByText(
        'Fixed toolbar second'
      )
    ).toBeVisible();

    first.view.unmount();
    expect(firstLayer.isConnected).toBe(false);
    expect(secondLayer.isConnected).toBe(true);
    expect(scrollOwner).toHaveStyle({ position: 'relative' });

    second.view.unmount();
    expect(secondLayer.isConnected).toBe(false);
    expect(scrollOwner).toHaveStyle({ position: 'static' });
  });
});

async function mountAffixSurface(
  onChange = vi.fn(),
  suffix = 'block',
  owner?: HTMLElement
) {
  const registry = createFrontstageNativeReactModuleRegistry();
  const antdModule = await registry.load('antd');
  const Affix = antdModule.Affix as ComponentType<AffixProps>;
  const scrollOwner = owner ?? document.createElement('div');
  scrollOwner.style.overflowY = 'auto';
  if (!owner) scrollOwner.style.position = 'static';
  const root = document.createElement('div');
  scrollOwner.append(root);
  if (!scrollOwner.isConnected) document.body.append(scrollOwner);
  const plan = createPlan(`native-affix-${suffix}`);
  const toolbarLabel =
    suffix === 'block' ? 'Fixed toolbar' : `Fixed toolbar ${suffix}`;
  const view = render(
    <FrontstageNativeTrustedBlockPortalHost
      root={root}
      renderEpoch="affix:1"
      plan={plan}
      component={() => (
        <Affix offsetTop={24} onChange={onChange}>
          <div style={{ height: 80 }}>{toolbarLabel}</div>
        </Affix>
      )}
      ctx={createContext()}
    />
  );
  const shadowRoot = await waitFor(() => {
    expect(root.shadowRoot).not.toBeNull();
    return root.shadowRoot as ShadowRoot;
  });
  return { root, scrollOwner, shadowRoot, plan, view };
}

async function findAffixLayer(
  fixture: Awaited<ReturnType<typeof mountAffixSurface>>
): Promise<HTMLElement> {
  return waitFor(() => {
    const layer = fixture.scrollOwner.querySelector<HTMLElement>(
      `[data-flowbase-native-affix-layer="${fixture.plan.blockId}"]`
    );
    expect(layer).not.toBeNull();
    expect(layer?.shadowRoot).not.toBeNull();
    return layer as HTMLElement;
  });
}

async function nextAnimationFrame(): Promise<void> {
  await new Promise<void>((resolve) =>
    window.requestAnimationFrame(() => resolve())
  );
}

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

function domRect({
  top,
  left,
  width,
  height
}: {
  top: number;
  left: number;
  width: number;
  height: number;
}): DOMRect {
  return {
    x: left,
    y: top,
    top,
    left,
    right: left + width,
    bottom: top + height,
    width,
    height,
    toJSON: () => ({})
  } as DOMRect;
}
