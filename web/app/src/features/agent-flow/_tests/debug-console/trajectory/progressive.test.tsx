import { act, renderHook } from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import { useProgressiveTrajectory } from '../../../components/debug-console/trajectory/use-progressive-trajectory';

afterEach(() => vi.useRealTimers());
test('continues only while visible and idle, pauses on failure, and cancels scheduled work on close', async () => {
  vi.useFakeTimers();
  const fetchNextPage = vi.fn().mockResolvedValue(undefined);
  const initial = {
    active: true,
    hasNextPage: true,
    isFetching: false,
    isError: false
  };
  const { rerender, unmount } = renderHook(
    ({ active, ...pages }) =>
      useProgressiveTrajectory(active, { ...pages, fetchNextPage }),
    { initialProps: initial }
  );
  await act(() => vi.advanceTimersByTimeAsync(100));
  expect(fetchNextPage).toHaveBeenCalledTimes(1);
  rerender({ ...initial, isFetching: true });
  await act(() => vi.advanceTimersByTimeAsync(500));
  expect(fetchNextPage).toHaveBeenCalledTimes(1);
  rerender({ ...initial, active: false });
  await act(() => vi.advanceTimersByTimeAsync(500));
  expect(fetchNextPage).toHaveBeenCalledTimes(1);
  rerender({ ...initial, isError: true });
  await act(() => vi.advanceTimersByTimeAsync(500));
  expect(fetchNextPage).toHaveBeenCalledTimes(1);
  rerender(initial);
  unmount();
  await act(() => vi.advanceTimersByTimeAsync(500));
  expect(fetchNextPage).toHaveBeenCalledTimes(1);
});
