import { act, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { useFrontstagePageCanvasWidth } from '../use-canvas-width';

const originalResizeObserver = globalThis.ResizeObserver;
afterEach(() => {
  globalThis.ResizeObserver = originalResizeObserver;
  vi.restoreAllMocks();
});
function Host() {
  const { width, containerRef } = useFrontstagePageCanvasWidth();
  return (
    <div ref={containerRef} data-testid="width">
      {width}
    </div>
  );
}
it('measures before paint, retains hidden geometry, then follows real resizing', () => {
  let report: ResizeObserverCallback | undefined;
  class Observer {
    constructor(callback: ResizeObserverCallback) {
      report = callback;
    }
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  globalThis.ResizeObserver = Observer as unknown as typeof ResizeObserver;
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(1735);
  render(<Host />);
  expect(screen.getByTestId('width').textContent).toBe('1735');
  const resize = (width: number) =>
    act(() => {
      report?.(
        [{ contentRect: { width } } as ResizeObserverEntry],
        {} as ResizeObserver
      );
    });
  resize(0);
  expect(screen.getByTestId('width').textContent).toBe('1735');
  resize(640);
  expect(screen.getByTestId('width').textContent).toBe('640');
  resize(0);
  resize(1834);
  expect(screen.getByTestId('width').textContent).toBe('1834');
});
it('does not fabricate a desktop allocation before first measurement', () => {
  globalThis.ResizeObserver = class {
    observe() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(0);
  render(<Host />);
  expect(screen.getByTestId('width').textContent).toBe('0');
});
