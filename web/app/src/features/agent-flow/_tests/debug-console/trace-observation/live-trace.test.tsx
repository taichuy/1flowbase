import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, expect, test, vi } from 'vitest';
import { LazyConversationTrace } from '../../../components/debug-console/LazyConversationTrace';
import type {
  ConversationLogTraceLoader,
  ConversationLogTraceNodeSummary
} from '../../../components/debug-console/conversation-log-trace-model';

const at = '2026-11-15T10:00:00Z';
function node(index: number, running = false): ConversationLogTraceNodeSummary {
  return {
    trace_node_id: `node-${index}`,
    node_kind: 'node_run',
    node_id: `node-${index}`,
    node_type: 'code',
    node_alias: `Step ${index}`,
    status: running ? 'running' : 'succeeded',
    started_at: at,
    finished_at: running ? null : at,
    has_children: false,
    has_content: true
  };
}
function mount(loader: ConversationLogTraceLoader) {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false, refetchIntervalInBackground: true }
    }
  });
  const view = render(
    <QueryClientProvider client={client}>
      <LazyConversationTrace
        runId="run"
        defaultToolsExpanded={false}
        traceLoader={loader}
      />
    </QueryClientProvider>
  );
  return { ...view, client };
}
afterEach(() => {
  vi.useRealTimers();
});

test('refreshes all loaded root pages until the backend flow ends, even when the first 20 nodes finished', async () => {
  vi.useFakeTimers();
  let ended = false;
  const first = Array.from({ length: 20 }, (_, i) => node(i));
  const loadTree = vi.fn(async () => ({
    flow_run: { id: 'run', status: ended ? 'failed' : 'running' },
    nodes: first,
    page_info: { has_more: true, next_cursor: 'later', page_size: 20 }
  }));
  const loadChildren = vi.fn(async () => ({
    items: [{ ...node(20, !ended), status: ended ? 'failed' : 'running' }],
    page_info: { has_more: false, next_cursor: null, page_size: 20 }
  }));
  await act(async () => {
    mount({ loadTree, loadChildren });
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: /加载更多/ }));
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  expect(screen.getByRole('button', { name: /Step 20/ })).toBeVisible();
  ended = true;
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1600);
  });
  const last = screen.getByRole('button', { name: /Step 20/ });
  expect(within(last).getByLabelText(/failed/)).toBeVisible();
  expect(loadChildren).toHaveBeenCalledWith('run', 'root', 'later');
  expect(loadChildren.mock.calls.length).toBeGreaterThan(1);
  const count = loadTree.mock.calls.length;
  await act(async () => {
    await vi.advanceTimersByTimeAsync(3000);
  });
  expect(loadTree).toHaveBeenCalledTimes(count);
});

test('refreshes every loaded round-child page and terminal summaries', async () => {
  vi.useFakeTimers();
  let ended = false;
  const parent = {
    ...node(99, !ended),
    node_kind: 'round_group',
    node_type: 'rounds',
    has_children: true,
    has_content: false
  };
  const loadTree = vi.fn(async () => ({
    flow_run: { id: 'run', status: ended ? 'failed' : 'running' },
    nodes: [
      {
        ...parent,
        status: ended ? 'failed' : 'running',
        finished_at: ended ? at : null
      }
    ]
  }));
  const loadChildren = vi.fn(
    async (_run: string, _node: string, cursor?: string) => ({
      items: cursor
        ? [{ ...node(20, !ended), status: ended ? 'failed' : 'running' }]
        : Array.from({ length: 20 }, (_, i) => node(i)),
      page_info: {
        has_more: !cursor,
        next_cursor: cursor ? null : 'later',
        page_size: 20
      }
    })
  );
  await act(async () => {
    mount({ loadTree, loadChildren });
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: /Step 99/ }));
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: /加载更多/ }));
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  ended = true;
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1600);
  });
  expect(
    within(screen.getByRole('button', { name: /Step 20/ })).getByLabelText(
      /failed/
    )
  ).toBeVisible();
  expect(loadChildren).toHaveBeenCalledWith('run', 'node-99', 'later');
});

test('content read failure is visible and retryable without changing execution status', async () => {
  const loadContent = vi.fn().mockRejectedValue(new Error('read failed'));
  mount({ loadTree: async () => ({ nodes: [node(1)] }), loadContent });
  fireEvent.click(await screen.findByRole('button', { name: /Step 1/ }));
  expect(await screen.findByRole('alert')).toHaveTextContent('加载失败');
  expect(
    within(screen.getByRole('button', { name: /Step 1/ })).getByLabelText(
      /succeeded/
    )
  ).toBeVisible();
  loadContent.mockResolvedValue({
    trace_node_id: 'node-1',
    node_kind: 'node_run',
    payload: { input_payload: { target: 'codex' } }
  });
  fireEvent.click(screen.getByRole('button', { name: /重\s*试/ }));
  expect(await screen.findByText('输入', { exact: true })).toBeVisible();
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});

test('group child read failure is visible and retryable', async () => {
  const loadChildren = vi.fn().mockRejectedValue(new Error('read failed'));
  mount({
    loadTree: async () => ({
      nodes: [
        {
          ...node(1),
          node_kind: 'round_group',
          node_type: 'rounds',
          has_content: false,
          has_children: true
        }
      ]
    }),
    loadChildren
  });
  fireEvent.click(await screen.findByRole('button', { name: /Step 1/ }));
  expect(await screen.findByRole('alert')).toHaveTextContent('加载失败');
  loadChildren.mockResolvedValue({
    items: [node(2)],
    page_info: { has_more: false, next_cursor: null, page_size: 20 }
  });
  fireEvent.click(screen.getByRole('button', { name: /重\s*试/ }));
  expect(await screen.findByRole('button', { name: /Step 2/ })).toBeVisible();
});
