import type { ComponentType } from 'react';
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import type { CountUpProps } from 'react-countup';
import { compileNativeReactComponent } from '@1flowbase/page-runtime';
import { createFrontstageNativeReactModuleRegistry } from '../registry';

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test('countup compiles and animates independent values, then cancels on unmount', async () => {
  const registry = createFrontstageNativeReactModuleRegistry();
  expect(compileNativeReactComponent(
    `import CountUp, { useCountUp } from 'react-countup';
export default function Block() { return <CountUp end={112893} separator="," />; }`,
    registry.definitions
  ).ok).toBe(true);
  const loaded = await registry.load('react-countup');
  expect(typeof loaded.useCountUp).toBe('function');
  const CountUp = loaded.default as ComponentType<CountUpProps>;
  vi.useFakeTimers();
  const view = render(<><CountUp end={112893} duration={1} separator="," decimals={2} /><CountUp end={42} duration={1} /></>);
  expect(view.container.textContent).not.toContain('112,893.00');
  act(() => { vi.advanceTimersByTime(1200); });
  expect(view.container.textContent).toBe('112,893.0042');
  view.rerender(<CountUp end={900000} duration={10} />);
  act(() => { vi.advanceTimersByTime(100); });
  expect(vi.getTimerCount()).toBeGreaterThan(0);
  view.unmount();
  expect(vi.getTimerCount()).toBe(0);
});
