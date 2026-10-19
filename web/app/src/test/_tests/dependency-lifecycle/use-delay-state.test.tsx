import { createRequire } from 'node:module';
import { StrictMode, useEffect, type SetStateAction } from 'react';
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

type Delay = boolean | { ms: number } | { frame: number };
type DelayStateHook = (initial: number | (() => number)) => [
  number,
  (next: SetStateAction<number>, delay?: Delay) => void
];
type HookModule = { default: DelayStateHook };

const require = createRequire(import.meta.url);
// Resolve the real installed transitive dependency through its actual consumer.
const requireFromAntd = createRequire(require.resolve('antd/package.json'));
const commonJS = requireFromAntd(
  '@rc-component/util/lib/hooks/useDelayState'
) as HookModule;
const esm = await vi.importActual<HookModule>(
  requireFromAntd.resolve('@rc-component/util/es/hooks/useDelayState.js')
);
const mountedViews = new Set<{ unmount(): void }>();

beforeEach(() => {
  vi.useFakeTimers({
    toFake: [
      'setTimeout',
      'clearTimeout',
      'requestAnimationFrame',
      'cancelAnimationFrame'
    ]
  });
});

afterEach(() => {
  for (const view of mountedViews) view.unmount();
  mountedViews.clear();
  vi.useRealTimers();
});

function dispose(view: { unmount(): void }) {
  view.unmount();
  mountedViews.delete(view);
}

test('loads distinct real CommonJS and ESM hook artifacts', () => {
  expect(commonJS.default).not.toBe(esm.default);
});

describe.each([
  { name: 'CommonJS', useDelayState: commonJS.default },
  { name: 'ESM', useDelayState: esm.default }
])('$name useDelayState lifecycle', ({ useDelayState }) => {
  function mount(initial: number | (() => number) = 0) {
    const view = renderHook(() => useDelayState(initial));
    mountedViews.add(view);
    return view;
  }

  test.each([
    { name: 'milliseconds', delay: { ms: 10 } },
    { name: 'zero-delay milliseconds', delay: { ms: 0 } },
    { name: 'multiple RAF frames', delay: { frame: 2 } },
    { name: 'default RAF', delay: undefined },
    { name: 'false RAF', delay: false }
  ] satisfies Array<{ name: string; delay: Delay | undefined }>)('cancels $name on unmount', ({ delay }) => {
    const view = mount();
    const update = vi.fn(() => 1);
    act(() => view.result.current[1](update, delay));
    expect(vi.getTimerCount()).toBe(1);

    dispose(view);
    // This assertion rejects the old hook even if React drops a stale dispatch.
    expect(vi.getTimerCount()).toBe(0);
    act(() => vi.advanceTimersByTime(64));
    expect(update).not.toHaveBeenCalled();
  });

  test.each([
    { name: 'milliseconds', delay: { ms: 20 } },
    { name: 'RAF', delay: { frame: 2 } }
  ] satisfies Array<{ name: string; delay: Delay }>)('replaces only the pending $name update', ({ delay }) => {
    const view = mount();
    const replaced = vi.fn(() => 1);
    act(() => {
      view.result.current[1](replaced, delay);
      view.result.current[1]((value) => value + 2, delay);
    });
    expect(vi.getTimerCount()).toBe(1);
    act(() => vi.advanceTimersByTime(64));
    expect(replaced).not.toHaveBeenCalled();
    expect(view.result.current[0]).toBe(2);
    expect(vi.getTimerCount()).toBe(0);
  });

  test('retains a pending update through rerender and commits at its requested deadline', () => {
    const view = mount();
    const setter = view.result.current[1];
    act(() => setter((value) => value + 1, { ms: 20 }));
    view.rerender();
    expect(view.result.current[1]).toBe(setter);
    expect(vi.getTimerCount()).toBe(1);
    act(() => vi.advanceTimersByTime(19));
    expect(view.result.current[0]).toBe(0);
    act(() => vi.advanceTimersByTime(1));
    expect(view.result.current[0]).toBe(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  test('keeps lazy initial state and immediate functional updates while cancelling the pending delay', () => {
    const view = mount(() => 2);
    const replaced = vi.fn(() => 99);
    act(() => {
      view.result.current[1](replaced, { ms: 20 });
      view.result.current[1]((value) => value + 3, true);
    });
    expect(view.result.current[0]).toBe(5);
    expect(vi.getTimerCount()).toBe(0);
    act(() => vi.advanceTimersByTime(64));
    expect(replaced).not.toHaveBeenCalled();
  });

  test('unmount cancels only its instance and leaves another instance scheduled', () => {
    const first = mount();
    const second = mount();
    const abandoned = vi.fn(() => 7);
    act(() => {
      first.result.current[1](abandoned, { ms: 20 });
      second.result.current[1](9, { frame: 2 });
    });
    expect(vi.getTimerCount()).toBe(2);
    dispose(first);
    expect(vi.getTimerCount()).toBe(1);
    act(() => vi.advanceTimersByTime(64));
    expect(abandoned).not.toHaveBeenCalled();
    expect(second.result.current[0]).toBe(9);
    expect(vi.getTimerCount()).toBe(0);
  });

  test('StrictMode effect replay can schedule again after cleanup without retaining a cancelled state', () => {
    const view = renderHook(() => {
      const [value, setValue] = useDelayState(0);
      useEffect(() => {
        setValue((current) => current + 1, { ms: 20 });
      }, [setValue]);
      return value;
    }, { wrapper: StrictMode });
    mountedViews.add(view);
    expect(vi.getTimerCount()).toBe(1);
    act(() => vi.advanceTimersByTime(20));
    expect(view.result.current).toBe(1);
    expect(vi.getTimerCount()).toBe(0);
    dispose(view);
    expect(vi.getTimerCount()).toBe(0);
  });
});
