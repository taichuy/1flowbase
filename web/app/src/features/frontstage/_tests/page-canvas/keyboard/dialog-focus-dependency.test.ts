import { createRequire } from 'node:module';
import { afterEach, describe, expect, test } from 'vitest';

// Exercise the installed CJS entry as well as the ESM Modal consumer in the
// canvas test. The dependency patch must ship both distribution formats.
const require = createRequire(import.meta.url);
const { lockFocus } = require(
  require.resolve('@rc-component/util', { paths: [require.resolve('antd')] })
) as { lockFocus(element: HTMLElement, id: string): () => void };

const disposals: Array<() => void> = [];
afterEach(() => {
  for (const dispose of disposals.splice(0).reverse()) dispose();
});

function fixture(shadow: boolean) {
  const host = document.createElement('div');
  document.body.append(host);
  disposals.push(() => host.remove());
  const root = shadow ? host.attachShadow({ mode: 'open' }) : host;
  const dialog = document.createElement('section');
  const first = document.createElement('input');
  const second = document.createElement('textarea');
  const outside = document.createElement('button');
  for (const control of [first, second, outside]) {
    Object.defineProperty(control, 'offsetParent', { get: () => host });
  }
  dialog.append(first, second);
  root.append(dialog, outside);
  return { root, dialog, first, second, outside };
}

function activeElement(): Element | null {
  let element = document.activeElement;
  while (element?.shadowRoot?.activeElement)
    element = element.shadowRoot.activeElement;
  return element;
}

describe('I2019 AC-004 installed dialog focus dependency', () => {
  test.each([false, true])(
    'preserves focused child and traps escape, shadow=%s',
    (shadow) => {
      const { dialog, first, second, outside } = fixture(shadow);
      second.focus();
      disposals.push(lockFocus(dialog, 'dialog'));
      expect(activeElement()).toBe(second);
      outside.focus();
      expect(activeElement()).toBe(second);
      first.focus();
      expect(activeElement()).toBe(first);
    }
  );

  test('only the top lock traps focus and releases to the underlying scope', () => {
    const outer = fixture(true);
    const inner = fixture(true);
    outer.second.focus();
    disposals.push(lockFocus(outer.dialog, 'outer'));
    const releaseInner = lockFocus(inner.dialog, 'inner');
    disposals.push(releaseInner);
    inner.second.focus();
    expect(activeElement()).toBe(inner.second);
    outer.outside.focus();
    expect(activeElement()).toBe(inner.second);
    releaseInner();
    outer.second.focus();
    expect(activeElement()).toBe(outer.second);
  });

  test('keeps a shared shadow-root listener until the last lock is released', () => {
    const { root, dialog, first, second, outside } = fixture(true);
    const inner = document.createElement('section');
    const innerInput = document.createElement('input');
    Object.defineProperty(innerInput, 'offsetParent', { get: () => inner });
    inner.append(innerInput);
    root.append(inner);
    second.focus();
    const releaseOuter = lockFocus(dialog, 'shared-outer');
    disposals.push(releaseOuter);
    const releaseInner = lockFocus(inner, 'shared-inner');
    disposals.push(releaseInner);
    releaseOuter();
    outside.focus();
    expect(activeElement()).toBe(innerInput);
    releaseInner();
    first.focus();
    expect(activeElement()).toBe(first);
  });

  test.each([false, true])(
    'wraps Tab and Shift+Tab inside the active scope, shadow=%s',
    (shadow) => {
      const { dialog, first, second, outside } = fixture(shadow);
      disposals.push(lockFocus(dialog, 'tab'));
      second.focus();
      second.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Tab',
          bubbles: true,
          composed: true
        })
      );
      outside.focus();
      expect(activeElement()).toBe(first);
      first.dispatchEvent(
        new KeyboardEvent('keydown', {
          key: 'Tab',
          shiftKey: true,
          bubbles: true,
          composed: true
        })
      );
      outside.focus();
      expect(activeElement()).toBe(second);
    }
  );

  test('recognizes a focused control in a nested shadow root as inside the dialog', () => {
    const { dialog } = fixture(true);
    const widget = document.createElement('div');
    const nested = widget.attachShadow({ mode: 'open' });
    const field = document.createElement('input');
    nested.append(field);
    dialog.append(widget);
    field.focus();
    disposals.push(lockFocus(dialog, 'nested'));
    expect(activeElement()).toBe(field);
  });
});
