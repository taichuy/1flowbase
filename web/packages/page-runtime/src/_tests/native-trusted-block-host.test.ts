// @vitest-environment jsdom

import '@testing-library/jest-dom/vitest';

import { describe, expect, test } from 'vitest';

import { attachNativeTrustedBlockPortalSurface } from '../index';

function createRoot(): HTMLDivElement {
  const root = document.createElement('div');
  document.body.append(root);
  return root;
}

describe('native trusted block portal surface', () => {
  test('D3R-AC-001 attaches an isolated ShadowRoot mount without owning React lifecycle', () => {
    const firstRoot = createRoot();
    const secondRoot = createRoot();

    const first = attachNativeTrustedBlockPortalSurface({
      root: firstRoot,
      blockId: 'first'
    });
    const second = attachNativeTrustedBlockPortalSurface({
      root: secondRoot,
      blockId: 'second'
    });

    expect(first.shadowRoot).not.toBe(second.shadowRoot);
    expect(first.mountElement.getRootNode()).toBe(first.shadowRoot);
    expect(second.mountElement.getRootNode()).toBe(second.shadowRoot);
    expect(first).not.toHaveProperty('render');
    expect(first).not.toHaveProperty('unmount');
    expect(first).not.toHaveProperty('update');
  });

  test('I1969-AC-001 I1969-AC-002 I1969-AC-008 exposes one unclipped flow root without a forced gutter boundary', () => {
    const root = createRoot();
    const surface = attachNativeTrustedBlockPortalSurface({
      root,
      blockId: 'flow-content'
    });

    expect(surface).not.toHaveProperty('slotElement');
    expect(
      surface.shadowRoot.querySelector(
        '[data-flowbase-native-trusted-block-slot]'
      )
    ).toBeNull();
    expect(surface.mountElement.parentNode).toBe(surface.shadowRoot);
    expect(surface.mountElement.style.padding).toBe('');
    expect(
      surface.mountElement.dataset.flowbaseNativeTrustedBlockAllocationMode
    ).toBeUndefined();
    expect(surface.mountElement).toHaveStyle({
      width: '100%',
      maxWidth: '100%',
      minWidth: '0',
      height: '100%',
      boxSizing: 'border-box',
      overflow: 'visible'
    });

    surface.dispose();
  });

  test('D3R-AC-008 cleanup is idempotent, restores host markers, and permits a fresh epoch', () => {
    const root = createRoot();
    root.setAttribute('data-flowbase-native-trusted-block-id', 'host-value');
    const first = attachNativeTrustedBlockPortalSurface({
      root,
      blockId: 'epoch-1'
    });
    const shadowRoot = first.shadowRoot;

    expect(root).toHaveAttribute('data-flowbase-native-trusted-block-root', '');
    expect(root).toHaveAttribute(
      'data-flowbase-native-trusted-block-id',
      'epoch-1'
    );

    first.dispose();
    first.dispose();

    expect(shadowRoot.childNodes).toHaveLength(0);
    expect(root).not.toHaveAttribute('data-flowbase-native-trusted-block-root');
    expect(root).toHaveAttribute(
      'data-flowbase-native-trusted-block-id',
      'host-value'
    );

    const second = attachNativeTrustedBlockPortalSurface({
      root,
      blockId: 'epoch-2'
    });
    expect(second.shadowRoot).toBe(shadowRoot);
    expect(second.mountElement).not.toBe(first.mountElement);
    second.dispose();
  });

  test('rejects invalid, pre-shadowed, and concurrently active roots', () => {
    expect(() =>
      attachNativeTrustedBlockPortalSurface({
        root: { nodeType: 1 } as Element,
        blockId: 'invalid'
      })
    ).toThrow('portal root must be a DOM Element');

    const preShadowed = createRoot();
    preShadowed.attachShadow({ mode: 'open' });
    expect(() =>
      attachNativeTrustedBlockPortalSurface({
        root: preShadowed,
        blockId: 'pre-shadowed'
      })
    ).toThrow('must not contain a pre-existing ShadowRoot');

    const activeRoot = createRoot();
    const active = attachNativeTrustedBlockPortalSurface({
      root: activeRoot,
      blockId: 'active'
    });
    expect(() =>
      attachNativeTrustedBlockPortalSurface({
        root: activeRoot,
        blockId: 'duplicate'
      })
    ).toThrow('root is already active');
    active.dispose();
  });
});
