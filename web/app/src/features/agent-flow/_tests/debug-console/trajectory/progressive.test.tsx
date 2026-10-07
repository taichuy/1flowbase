import './navigation';
import {
  QueryClient,
  QueryClientProvider,
  useInfiniteQuery
} from '@tanstack/react-query';
import type { ReactNode } from 'react';
import {
  act,
  render,
  renderHook,
  screen,
  waitFor
} from '@testing-library/react';
import { afterEach, expect, test, vi } from 'vitest';
import type { WorkflowTrajectoryEvent } from '@1flowbase/api-client';
import { NativeTrajectoryWorkspace } from '../../../components/debug-console/trajectory/NativeTrajectoryWorkspace';
import { workflowPage } from './workflow-fixture';
import { useProgressiveTrajectory } from '../../../components/debug-console/trajectory/use-progressive-trajectory';

afterEach(() => vi.useRealTimers());
test('continues only while visible and idle, pauses on failure, and cancels scheduled work on close', async () => {
  vi.useFakeTimers();
  const fetchNextPage = vi.fn().mockResolvedValue(undefined);
  const initial = {
    active: true,
    hasNextPage: true,
    isFetching: false,
    isError: false,
    data: { pages: [{}] }
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
  rerender({ ...initial, active: false, data: { pages: [{}, {}] } });
  await act(() => vi.advanceTimersByTimeAsync(500));
  expect(fetchNextPage).toHaveBeenCalledTimes(1);
  rerender({ ...initial, isError: true, data: { pages: [{}, {}, {}] } });
  await act(() => vi.advanceTimersByTimeAsync(500));
  expect(fetchNextPage).toHaveBeenCalledTimes(1);
  rerender(initial);
  unmount();
  await act(() => vi.advanceTimersByTimeAsync(500));
  expect(fetchNextPage).toHaveBeenCalledTimes(1);
});

test('immediately completed Native numeric pages keep scheduling even when fetching booleans are batched', async () => {
  const fetchNativePage = vi
    .fn()
    .mockImplementation((cursor: number | undefined) =>
      Promise.resolve({
        items: [{ id: cursor ?? 0 }],
        next_cursor: cursor === undefined ? 10 : cursor === 10 ? 11 : null
      })
    );
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const { result, unmount } = renderHook(
    () => {
      const pages = useInfiniteQuery({
        queryKey: ['native-immediate-pages'],
        initialPageParam: undefined as number | undefined,
        queryFn: ({ pageParam }) => fetchNativePage(pageParam),
        getNextPageParam: (page) => page.next_cursor ?? undefined
      });
      useProgressiveTrajectory(true, pages);
      return pages;
    },
    { wrapper }
  );
  await waitFor(() => expect(result.current.data?.pages).toHaveLength(3));
  expect(fetchNativePage.mock.calls.map(([cursor]) => cursor)).toEqual([
    undefined,
    10,
    11
  ]);
  expect(result.current.hasNextPage).toBe(false);
  unmount();
  client.clear();
});

test('an immediately rejected next Native page stops serial scheduling', async () => {
  const fetchNativePage = vi
    .fn()
    .mockImplementation((cursor: number | undefined) =>
      cursor === undefined
        ? Promise.resolve({ items: [], next_cursor: 10 })
        : Promise.reject(new Error('Native page unavailable'))
    );
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  const { result, unmount } = renderHook(
    () => {
      const pages = useInfiniteQuery({
        queryKey: ['native-immediate-error'],
        initialPageParam: undefined as number | undefined,
        queryFn: ({ pageParam }) => fetchNativePage(pageParam),
        getNextPageParam: (page) => page.next_cursor ?? undefined
      });
      useProgressiveTrajectory(true, pages);
      return pages;
    },
    { wrapper }
  );
  await waitFor(() => expect(result.current.isError).toBe(true));
  expect(fetchNativePage.mock.calls.map(([cursor]) => cursor)).toEqual([
    undefined,
    10
  ]);
  expect(result.current.data?.pages).toHaveLength(1);
  unmount();
  client.clear();
});

test('the existing Native workspace renders all three immediately completed summary pages', async () => {
  const event = (index: number): WorkflowTrajectoryEvent => ({
    event_id: `native-${index}`,
    event_sequence: index,
    event_type: 'node_started',
    created_at: `2026-09-22T00:0${index}:00Z`,
    category: 'nodes',
    flow_run_id: 'run-current',
    task_run_id: 'task-root',
    parent_task_run_id: null,
    node_run_id: 'node-current',
    node_id: 'llm',
    node_alias: 'Native node',
    node_type: 'llm',
    status: 'running',
    preview: `native-page-${index}`,
    native_step: null
  });
  const loadWorkflowTrajectory = vi
    .fn()
    .mockImplementation((_runId, cursor) =>
      Promise.resolve(
        cursor === undefined
          ? workflowPage([event(1)], 'page-one')
          : cursor === 'page-one'
            ? workflowPage([event(2)], 'page-two')
            : workflowPage([event(3)])
      )
    );
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  const view = render(
    <QueryClientProvider client={client}>
      <NativeTrajectoryWorkspace
        runId="run-current"
        loader={{ loadWorkflowTrajectory }}
      />
    </QueryClientProvider>
  );
  expect(await screen.findByText('native-page-3')).toBeInTheDocument();
  expect(screen.getByText('native-page-1')).toBeInTheDocument();
  expect(screen.getByText('native-page-2')).toBeInTheDocument();
  expect(loadWorkflowTrajectory.mock.calls.map(([, cursor]) => cursor)).toEqual(
    [undefined, 'page-one', 'page-two']
  );
  view.unmount();
  client.clear();
});
