import { fireEvent, render, waitFor, within } from '@testing-library/react';
import type { ComponentType, MouseEvent } from 'react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import type { BlockContextSeed } from '@1flowbase/page-protocol';
import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';
import type { AnchorProps } from 'antd';

import { createFrontstageNativeReactModuleRegistry } from '../../lib/native-modules/registry';
import { resolveNativeBlockScrollOwner } from '../../lib/native-modules/native-block-surface-context';
import { FrontstageNativeTrustedBlockPortalHost } from '../../lib/native-trusted-block-react-adapter';

describe('native block Anchor runtime adapter', () => {
  beforeEach(() => {
    window.history.replaceState(null, '', '/native-anchor-test');
  });

  afterEach(() => {
    document.body.replaceChildren();
  });

  test('I1910-AC-001/005 resolves a local ShadowRoot target and keeps page history unchanged', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const antdModule = await registry.load('antd');
    const Anchor = antdModule.Anchor as ComponentType<AnchorProps>;
    const scrollOwner = document.createElement('div');
    scrollOwner.style.overflowY = 'auto';
    const root = document.createElement('div');
    scrollOwner.append(root);
    document.body.append(scrollOwner);
    const scrollTo = vi.fn((options: ScrollToOptions) => {
      Object.defineProperty(scrollOwner, 'scrollTop', {
        configurable: true,
        value: options.top ?? 0,
        writable: true
      });
    });
    Object.defineProperty(scrollOwner, 'scrollTo', {
      configurable: true,
      value: scrollTo
    });

    render(
      <FrontstageNativeTrustedBlockPortalHost
        root={root}
        renderEpoch="anchor:1"
        plan={createPlan()}
        component={() => (
          <>
            <div id="part-1">Part one content</div>
            <div id="part-2">Part two content</div>
            <Anchor
              items={[
                { key: 'part-1', href: '#part-1', title: 'Part 1' },
                { key: 'part-2', href: '#part-2', title: 'Part 2' }
              ]}
            />
          </>
        )}
        ctx={createContext()}
      />
    );

    const shadowRoot = await waitFor(() => {
      expect(root.shadowRoot).not.toBeNull();
      return root.shadowRoot as ShadowRoot;
    });
    const target = await within(
      shadowRoot as unknown as HTMLElement
    ).findByText('Part two content');
    expect(
      shadowRoot.querySelector('[data-flowbase-native-anchor-affix]')
    ).not.toBeNull();
    target.getBoundingClientRect = () => domRect({ top: 480, height: 120 });
    scrollOwner.getBoundingClientRect = () => domRect({ top: 80, height: 240 });
    const layer = await waitFor(() => {
      const element = scrollOwner.querySelector<HTMLElement>(
        '[data-flowbase-native-anchor-affix-layer="native-anchor-block"]'
      );
      expect(element?.shadowRoot).not.toBeNull();
      return element as HTMLElement;
    });

    fireEvent.click(
      await within(layer.shadowRoot as unknown as HTMLElement).findByRole(
        'link',
        { name: 'Part 2' }
      )
    );

    await waitFor(() => expect(scrollTo).toHaveBeenCalled());
    expect(scrollTo).toHaveBeenLastCalledWith(
      expect.objectContaining({ top: 400 })
    );
    expect(window.location.hash).toBe('');
  });

  test('I1910-AC-002 updates the active item from the real scroll owner', async () => {
    const fixture = await mountAnchorSurface();
    const firstTarget = fixture.shadowRoot.getElementById('part-1');
    const secondTarget = fixture.shadowRoot.getElementById('part-2');
    if (!firstTarget || !secondTarget)
      throw new Error('Missing anchor target.');
    fixture.scrollOwner.getBoundingClientRect = () =>
      domRect({ top: 80, height: 240 });
    fixture.scrollOwner.style.transform = 'translate3d(0, 0, 0)';
    firstTarget.getBoundingClientRect = () =>
      domRect({ top: 80 - fixture.scrollOwner.scrollTop, height: 120 });
    secondTarget.getBoundingClientRect = () =>
      domRect({ top: 480 - fixture.scrollOwner.scrollTop, height: 120 });

    fireEvent.scroll(fixture.scrollOwner);
    const firstLink = within(
      fixture.anchorRoot as unknown as HTMLElement
    ).getByRole('link', { name: 'Part 1' });
    await waitFor(() =>
      expect(firstLink.className).toContain('-link-title-active')
    );

    fixture.scrollOwner.scrollTop = 400;
    fireEvent.scroll(fixture.scrollOwner);
    const secondLink = within(
      fixture.anchorRoot as unknown as HTMLElement
    ).getByRole('link', { name: 'Part 2' });
    await waitFor(() =>
      expect(secondLink.className).toContain('-link-title-active')
    );
  });

  test('I1910-AC-009/011 delegates affix geometry to a surface-owned layer past the Block boundary', async () => {
    const fixture = await mountAnchorSurface('affix');
    const placeholder = fixture.shadowRoot.querySelector<HTMLElement>(
      '[data-flowbase-native-anchor-affix]'
    );
    const sentinel = fixture.shadowRoot.querySelector<HTMLElement>(
      '[data-flowbase-native-anchor-affix-sentinel]'
    );
    if (!placeholder || !sentinel) {
      throw new Error('Missing Anchor affix placeholder.');
    }
    fixture.scrollOwner.getBoundingClientRect = () =>
      domRect({ top: 80, height: 240 });
    fixture.root.getBoundingClientRect = () =>
      domRect({ top: -800, height: 600 });
    placeholder.getBoundingClientRect = () =>
      domRect({ top: -220, height: 94 });
    sentinel.getBoundingClientRect = () => domRect({ top: -220, height: 0 });

    fireEvent.scroll(fixture.scrollOwner);
    await nextAnimationFrame();

    const layer = await waitFor(() => {
      const element = fixture.scrollOwner.querySelector<HTMLElement>(
        `[data-flowbase-native-anchor-affix-layer="${fixture.plan.blockId}"]`
      );
      expect(element).not.toBeNull();
      return element as HTMLElement;
    });
    expect(layer.parentElement).toBe(fixture.scrollOwner);
    expect(fixture.root.contains(layer)).toBe(false);
    expect(layer).toHaveAttribute('data-flowbase-native-anchor-pinned', 'true');
    expect(layer).toHaveStyle({ position: 'sticky', top: '0px' });
    expect(layer.shadowRoot?.textContent).toContain('Part 1');
  });

  test('I1910-AC-008 keeps a monotonic scroll in the pinned state across geometry feedback', async () => {
    const onChange = vi.fn();
    const fixture = await mountAnchorSurface('affix-hysteresis', {
      affix: { onChange }
    });
    const affixTrack = fixture.shadowRoot.querySelector<HTMLElement>(
      '[data-flowbase-native-anchor-affix]'
    );
    const sentinel = fixture.shadowRoot.querySelector<HTMLElement>(
      '[data-flowbase-native-anchor-affix-sentinel]'
    );
    const layer = await findAffixLayer(fixture);
    if (!affixTrack || !sentinel) {
      throw new Error('Missing Anchor affix shell.');
    }
    fixture.scrollOwner.style.transform = 'translate3d(0, 0, 0)';
    fixture.scrollOwner.getBoundingClientRect = () =>
      domRect({ top: 80, height: 240 });
    sentinel.getBoundingClientRect = () =>
      domRect({ top: 100 - fixture.scrollOwner.scrollTop, height: 0 });
    layer.getBoundingClientRect = () => domRect({ top: 80, height: 94 });

    for (const scrollTop of [30, 32, 34]) {
      fixture.scrollOwner.scrollTop = scrollTop;
      fireEvent.scroll(fixture.scrollOwner);
      await nextAnimationFrame();
    }

    expect(onChange.mock.calls.map(([affixed]) => affixed)).toEqual([true]);
  });

  test('I1910-AC-010 does not re-scroll a horizontal Anchor after a semantic no-op rerender', async () => {
    const fixture = await mountAnchorSurface('stable-horizontal', {
      direction: 'horizontal'
    });
    const firstTarget = fixture.shadowRoot.getElementById('part-1');
    const secondTarget = fixture.shadowRoot.getElementById('part-2');
    if (!firstTarget || !secondTarget) {
      throw new Error('Missing anchor target.');
    }
    fixture.scrollOwner.getBoundingClientRect = () =>
      domRect({ top: 80, height: 240 });
    firstTarget.getBoundingClientRect = () =>
      domRect({ top: -320, height: 120 });
    secondTarget.getBoundingClientRect = () =>
      domRect({ top: 80, height: 120 });
    Object.defineProperties(fixture.scrollOwner, {
      clientHeight: { configurable: true, value: 240 },
      clientWidth: { configurable: true, value: 300 },
      scrollHeight: { configurable: true, value: 1_000 },
      scrollWidth: { configurable: true, value: 300 }
    });
    const secondLink = within(
      fixture.anchorRoot as unknown as HTMLElement
    ).getByRole('link', { name: 'Part 2' });
    secondLink.getBoundingClientRect = () => domRect({ top: 500, height: 24 });

    fireEvent.click(secondLink);
    await waitFor(() => expect(fixture.scroll).toHaveBeenCalled());
    await nextAnimationFrame();
    fixture.scroll.mockClear();

    fixture.rerender();
    await nextAnimationFrame();
    expect(
      within(fixture.anchorRoot as unknown as HTMLElement).getByRole('link', {
        name: 'Part 2'
      })
    ).toBe(secondLink);
    expect(fixture.scroll).not.toHaveBeenCalled();
  });

  test('I1910-AC-012 lets the most recently entered Anchor own the surface layer and restores the previous owner on reverse scroll', async () => {
    const scrollOwner = document.createElement('div');
    scrollOwner.style.overflowY = 'auto';
    document.body.append(scrollOwner);
    const first = await mountAnchorSurface('takeover-first', {}, scrollOwner);
    const second = await mountAnchorSurface('takeover-second', {}, scrollOwner);
    scrollOwner.getBoundingClientRect = () => domRect({ top: 80, height: 240 });
    const firstSentinel = first.shadowRoot.querySelector<HTMLElement>(
      '[data-flowbase-native-anchor-affix-sentinel]'
    );
    const secondSentinel = second.shadowRoot.querySelector<HTMLElement>(
      '[data-flowbase-native-anchor-affix-sentinel]'
    );
    if (!firstSentinel || !secondSentinel) {
      throw new Error('Missing Anchor takeover sentinels.');
    }
    firstSentinel.getBoundingClientRect = () =>
      domRect({ top: 100 - scrollOwner.scrollTop, height: 0 });
    secondSentinel.getBoundingClientRect = () =>
      domRect({ top: 500 - scrollOwner.scrollTop, height: 0 });

    scrollOwner.scrollTop = 200;
    fireEvent.scroll(scrollOwner);
    await nextAnimationFrame();
    const firstLayer = await findAffixLayer(first);
    const secondLayer = await findAffixLayer(second);
    expect(firstLayer).toHaveAttribute(
      'data-flowbase-native-anchor-pinned',
      'true'
    );
    expect(secondLayer).toHaveAttribute(
      'data-flowbase-native-anchor-pinned',
      'false'
    );

    scrollOwner.scrollTop = 600;
    fireEvent.scroll(scrollOwner);
    await nextAnimationFrame();
    expect(firstLayer).toHaveAttribute(
      'data-flowbase-native-anchor-pinned',
      'false'
    );
    expect(secondLayer).toHaveAttribute(
      'data-flowbase-native-anchor-pinned',
      'true'
    );

    scrollOwner.scrollTop = 200;
    fireEvent.scroll(scrollOwner);
    await nextAnimationFrame();
    expect(firstLayer).toHaveAttribute(
      'data-flowbase-native-anchor-pinned',
      'true'
    );
    expect(secondLayer).toHaveAttribute(
      'data-flowbase-native-anchor-pinned',
      'false'
    );
  });

  test('I1910-AC-003 chooses a fixed viewport before the page scroll owner', () => {
    const pageScrollOwner = document.createElement('main');
    pageScrollOwner.dataset.flowbaseFrontstageScrollOwner = '';
    const fixedViewport = document.createElement('section');
    fixedViewport.style.overflowY = 'auto';
    const root = document.createElement('div');
    fixedViewport.append(root);
    pageScrollOwner.append(fixedViewport);
    document.body.append(pageScrollOwner);

    expect(resolveNativeBlockScrollOwner(root)).toBe(fixedViewport);

    fixedViewport.style.overflowY = 'visible';
    expect(resolveNativeBlockScrollOwner(root)).toBe(pageScrollOwner);
  });

  test('I1910-AC-004 keeps duplicate target ids isolated by ShadowRoot', async () => {
    const first = await mountAnchorSurface('first');
    const second = await mountAnchorSurface('second');
    const firstTarget = first.shadowRoot.getElementById('part-2');
    const secondTarget = second.shadowRoot.getElementById('part-2');
    if (!firstTarget || !secondTarget)
      throw new Error('Missing anchor target.');
    first.scrollOwner.getBoundingClientRect = () =>
      domRect({ top: 0, height: 200 });
    second.scrollOwner.getBoundingClientRect = () =>
      domRect({ top: 0, height: 200 });
    firstTarget.getBoundingClientRect = () =>
      domRect({ top: 300, height: 100 });
    secondTarget.getBoundingClientRect = () =>
      domRect({ top: 700, height: 100 });

    fireEvent.click(
      within(second.anchorRoot as unknown as HTMLElement).getByRole('link', {
        name: 'Part 2'
      })
    );

    await waitFor(() => expect(second.scrollTo).toHaveBeenCalled());
    expect(second.scrollTo).toHaveBeenLastCalledWith(
      expect.objectContaining({ top: 700 })
    );
    expect(first.scrollTo).not.toHaveBeenCalled();
  });

  test('I1910-AC-005 respects a user-cancelled Anchor click', async () => {
    const onClick = vi.fn((event: MouseEvent<HTMLElement>) =>
      event.preventDefault()
    );
    const fixture = await mountAnchorSurface('cancelled', { onClick });
    const target = fixture.shadowRoot.getElementById('part-2');
    if (!target) throw new Error('Missing anchor target.');
    target.getBoundingClientRect = () => domRect({ top: 500, height: 100 });

    fireEvent.click(
      within(fixture.anchorRoot as unknown as HTMLElement).getByRole('link', {
        name: 'Part 2'
      })
    );

    expect(onClick).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({ href: '#part-2' })
    );
    expect(fixture.scrollTo).not.toHaveBeenCalled();
    expect(window.location.hash).toBe('');
  });

  test('I1910-AC-007 disposes the surface listener, layer, and owner lease', async () => {
    const scrollOwner = document.createElement('div');
    scrollOwner.style.position = 'static';
    const fixture = await mountAnchorSurface('dispose', {}, scrollOwner);
    const layer = await findAffixLayer(fixture);
    const removeEventListener = vi.spyOn(
      fixture.scrollOwner,
      'removeEventListener'
    );
    expect(fixture.scrollOwner).toHaveStyle({ position: 'relative' });

    fixture.view.unmount();

    expect(removeEventListener).toHaveBeenCalledWith(
      'scroll',
      expect.any(Function)
    );
    expect(layer.isConnected).toBe(false);
    expect(fixture.scrollOwner.style.position).toBe('static');
  });
});

async function mountAnchorSurface(
  suffix = 'default',
  anchorProps: Pick<AnchorProps, 'affix' | 'direction' | 'onClick'> = {},
  owner?: HTMLElement
) {
  const registry = createFrontstageNativeReactModuleRegistry();
  const antdModule = await registry.load('antd');
  const Anchor = antdModule.Anchor as ComponentType<AnchorProps>;
  const scrollOwner = owner ?? document.createElement('div');
  scrollOwner.style.overflowY = 'auto';
  const root = document.createElement('div');
  scrollOwner.append(root);
  if (!scrollOwner.isConnected) document.body.append(scrollOwner);
  const scrollTo = vi.fn();
  const scroll = vi.fn();
  Object.defineProperty(scrollOwner, 'scrollTo', {
    configurable: true,
    value: scrollTo
  });
  Object.defineProperty(scrollOwner, 'scroll', {
    configurable: true,
    value: scroll
  });
  const plan = { ...createPlan(), blockId: `native-anchor-${suffix}` };
  const ctx = createContext();
  const BlockComponent = () => (
    <>
      <div id="part-1">Part one content</div>
      <div id="part-2">Part two content</div>
      <Anchor
        items={[
          { key: 'part-1', href: '#part-1', title: 'Part 1' },
          { key: 'part-2', href: '#part-2', title: 'Part 2' }
        ]}
        {...anchorProps}
      />
    </>
  );
  const renderSurface = () => (
    <FrontstageNativeTrustedBlockPortalHost
      root={root}
      renderEpoch={`anchor:${suffix}`}
      plan={plan}
      component={BlockComponent}
      ctx={ctx}
    />
  );
  const view = render(renderSurface());
  const shadowRoot = await waitFor(() => {
    expect(root.shadowRoot).not.toBeNull();
    return root.shadowRoot as ShadowRoot;
  });
  const layer = await waitFor(() => {
    const element = scrollOwner.querySelector<HTMLElement>(
      `[data-flowbase-native-anchor-affix-layer="${plan.blockId}"]`
    );
    expect(element?.shadowRoot).not.toBeNull();
    return element as HTMLElement;
  });
  const anchorRoot = layer.shadowRoot as ShadowRoot;
  await within(anchorRoot as unknown as HTMLElement).findByRole('link', {
    name: 'Part 2'
  });
  return {
    root,
    scrollOwner,
    scroll,
    scrollTo,
    anchorRoot,
    shadowRoot,
    plan,
    view,
    rerender: () => view.rerender(renderSurface())
  };
}

async function findAffixLayer(
  fixture: Awaited<ReturnType<typeof mountAnchorSurface>>
): Promise<HTMLElement> {
  return waitFor(() => {
    const layer = fixture.scrollOwner.querySelector<HTMLElement>(
      `[data-flowbase-native-anchor-affix-layer="${fixture.plan.blockId}"]`
    );
    expect(layer).not.toBeNull();
    return layer as HTMLElement;
  });
}

async function nextAnimationFrame(): Promise<void> {
  await new Promise<void>((resolve) =>
    window.requestAnimationFrame(() => resolve())
  );
}

function createPlan(): NativeTrustedBlockPreparePlan {
  const source = 'export default function Block() { return null; }';
  return {
    runtime: 'native_trusted_block',
    blockId: 'native-anchor-block',
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

function domRect({ top, height }: { top: number; height: number }): DOMRect {
  return {
    x: 0,
    y: top,
    top,
    left: 0,
    right: 100,
    bottom: top + height,
    width: 100,
    height,
    toJSON: () => ({})
  } as DOMRect;
}
