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

    fireEvent.click(
      await within(shadowRoot as unknown as HTMLElement).findByRole('link', {
        name: 'Part 2'
      })
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
    if (!firstTarget || !secondTarget) throw new Error('Missing anchor target.');
    fixture.scrollOwner.getBoundingClientRect = () =>
      domRect({ top: 80, height: 240 });
    fixture.scrollOwner.style.transform = 'translate3d(0, 0, 0)';
    firstTarget.getBoundingClientRect = () =>
      domRect({ top: 80 - fixture.scrollOwner.scrollTop, height: 120 });
    secondTarget.getBoundingClientRect = () =>
      domRect({ top: 480 - fixture.scrollOwner.scrollTop, height: 120 });

    fireEvent.scroll(fixture.scrollOwner);
    const firstLink = within(
      fixture.shadowRoot as unknown as HTMLElement
    ).getByRole('link', { name: 'Part 1' });
    await waitFor(() =>
      expect(firstLink.className).toContain('-link-title-active')
    );

    fixture.scrollOwner.scrollTop = 400;
    fireEvent.scroll(fixture.scrollOwner);
    const secondLink = within(
      fixture.shadowRoot as unknown as HTMLElement
    ).getByRole('link', { name: 'Part 2' });
    await waitFor(() =>
      expect(secondLink.className).toContain('-link-title-active')
    );
  });

  test('I1910-AC-001 keeps an affixed Anchor visible inside a transformed grid item', async () => {
    const fixture = await mountAnchorSurface('affix');
    const affixTrack = fixture.shadowRoot.querySelector<HTMLElement>(
      '[data-flowbase-native-anchor-affix]'
    );
    const affix = affixTrack?.firstElementChild as HTMLElement | null;
    if (!affixTrack || !affix) throw new Error('Missing Anchor affix shell.');
    fixture.scrollOwner.getBoundingClientRect = () =>
      domRect({ top: 80, height: 240 });
    fixture.root.getBoundingClientRect = () =>
      domRect({ top: 20, height: 800 });
    affixTrack.getBoundingClientRect = () =>
      domRect({ top: 40, height: 94 });
    affix.getBoundingClientRect = () => domRect({ top: 40, height: 94 });

    fireEvent.scroll(fixture.scrollOwner);

    /* eslint-disable jest-dom/prefer-to-have-style -- toHaveStyle cannot inspect imperative styles inside this test ShadowRoot. */
    await waitFor(() =>
      expect(affix).toHaveAttribute(
        'style',
        expect.stringContaining('position: fixed')
      )
    );
    expect(affix).toHaveAttribute('style', expect.stringContaining('top: 0px'));
    expect(affixTrack).toHaveAttribute(
      'style',
      expect.stringContaining('height: 94px')
    );
    /* eslint-enable jest-dom/prefer-to-have-style */
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
    if (!firstTarget || !secondTarget) throw new Error('Missing anchor target.');
    first.scrollOwner.getBoundingClientRect = () =>
      domRect({ top: 0, height: 200 });
    second.scrollOwner.getBoundingClientRect = () =>
      domRect({ top: 0, height: 200 });
    firstTarget.getBoundingClientRect = () =>
      domRect({ top: 300, height: 100 });
    secondTarget.getBoundingClientRect = () =>
      domRect({ top: 700, height: 100 });

    fireEvent.click(
      within(second.shadowRoot as unknown as HTMLElement).getByRole('link', {
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
    const fixture = await mountAnchorSurface('cancelled', onClick);
    const target = fixture.shadowRoot.getElementById('part-2');
    if (!target) throw new Error('Missing anchor target.');
    target.getBoundingClientRect = () => domRect({ top: 500, height: 100 });

    fireEvent.click(
      within(fixture.shadowRoot as unknown as HTMLElement).getByRole('link', {
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

  test('I1910-AC-007 removes the surface scroll listener on dispose', async () => {
    const fixture = await mountAnchorSurface('dispose');
    const removeEventListener = vi.spyOn(
      fixture.scrollOwner,
      'removeEventListener'
    );

    fixture.view.unmount();

    expect(removeEventListener).toHaveBeenCalledWith(
      'scroll',
      expect.any(Function)
    );
  });
});

async function mountAnchorSurface(
  suffix = 'default',
  onClick?: AnchorProps['onClick']
) {
  const registry = createFrontstageNativeReactModuleRegistry();
  const antdModule = await registry.load('antd');
  const Anchor = antdModule.Anchor as ComponentType<AnchorProps>;
  const scrollOwner = document.createElement('div');
  scrollOwner.style.overflowY = 'auto';
  const root = document.createElement('div');
  scrollOwner.append(root);
  document.body.append(scrollOwner);
  const scrollTo = vi.fn();
  Object.defineProperty(scrollOwner, 'scrollTo', {
    configurable: true,
    value: scrollTo
  });
  const view = render(
    <FrontstageNativeTrustedBlockPortalHost
      root={root}
      renderEpoch={`anchor:${suffix}`}
      plan={{ ...createPlan(), blockId: `native-anchor-${suffix}` }}
      component={() => (
        <>
          <div id="part-1">Part one content</div>
          <div id="part-2">Part two content</div>
          <Anchor
            items={[
              { key: 'part-1', href: '#part-1', title: 'Part 1' },
              { key: 'part-2', href: '#part-2', title: 'Part 2' }
            ]}
            onClick={onClick}
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
  await within(shadowRoot as unknown as HTMLElement).findByRole('link', {
    name: 'Part 2'
  });
  return { root, scrollOwner, scrollTo, shadowRoot, view };
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

function domRect({
  top,
  height
}: {
  top: number;
  height: number;
}): DOMRect {
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
