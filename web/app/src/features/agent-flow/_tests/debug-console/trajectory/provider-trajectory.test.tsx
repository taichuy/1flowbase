import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, expect, test, vi } from 'vitest';
import { ProviderTrajectory } from '../../../components/debug-console/trajectory/ProviderTrajectory';
import { groupTraceItemsForDisplay } from '../../../components/debug-console/conversation/debug-workflow-trace-utils';
import type { ConversationLogTraceLoader } from '../../../components/debug-console/conversation-log-trace-model';
import type { AgentFlowTraceItem } from '../../../api/runtime';
import { appI18n } from '../../../../../shared/i18n/app-i18n';

beforeEach(async () => {
  await appI18n.changeLanguage('zh_Hans');
});
function fixture() {
  const loadTrajectory = vi.fn().mockResolvedValue({
    items: [
      {
        event_id: 'event-1',
        event_sequence: 7,
        event_type: 'provider_protocol_observation',
        created_at: '2026-09-21T01:00:00Z',
        metadata: {
          protocol: 'openai',
          transport: 'http',
          direction: 'sent',
          kind: 'request',
          encoding: 'utf8',
          flow_run_id: 'run-1',
          node_run_id: 'node-run-2',
          invocation_id: 'invocation-1',
          provider_attempt_index: 0,
          sequence: 1
        }
      }
    ],
    next_cursor: 7,
    observation_count: 2,
    persist_failed_count: 0,
    integrity: 'complete'
  });
  const loadTrajectoryBody = vi.fn().mockResolvedValue({
    event_id: 'event-1',
    body: '  { "provider": "original" }\n',
    encoding: 'utf8'
  });
  const loader: ConversationLogTraceLoader = {
    loadTree: vi.fn(),
    loadChildren: vi.fn(),
    loadContent: vi.fn(),
    loadTrajectory,
    loadTrajectoryBody
  };
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <ProviderTrajectory
        runId="run-1"
        nodeRunId="node-run-2"
        loader={loader}
        executionContent={<span>route / fusion execution</span>}
      />
    </QueryClientProvider>
  );
  return { loadTrajectory, loadTrajectoryBody };
}
test('loads a bounded summary page only on opening and the exact original body only on selection', async () => {
  const { loadTrajectory, loadTrajectoryBody } = fixture();
  expect(loadTrajectory).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '供应商轨迹' }));
  fireEvent.click(await screen.findByRole('button', { name: '请求' }));
  await waitFor(() =>
    expect(loadTrajectoryBody).toHaveBeenCalledWith(
      'run-1',
      'node-run-2',
      'event-1'
    )
  );
  const body = await screen.findByText(/"provider": "original"/);
  expect(body.textContent).toBe('  { "provider": "original" }\n');
  expect(loadTrajectory).toHaveBeenCalledWith('run-1', 'node-run-2', undefined);
  fireEvent.click(screen.getByRole('tab', { name: '执行关联' }));
  expect(screen.getByText('route / fusion execution')).toBeTruthy();
});
test('does not fetch protocol bodies while browsing summary pages', async () => {
  const { loadTrajectory, loadTrajectoryBody } = fixture();
  fireEvent.click(screen.getByRole('button', { name: '供应商轨迹' }));
  await screen.findByRole('button', { name: '请求' });
  expect(loadTrajectoryBody).not.toHaveBeenCalled();
  loadTrajectory.mockResolvedValueOnce({
    items: [],
    next_cursor: null,
    observation_count: 2,
    persist_failed_count: 0,
    integrity: 'complete'
  });
  fireEvent.click(screen.getByRole('button', { name: '加载更多步骤' }));
  await waitFor(() =>
    expect(loadTrajectory).toHaveBeenCalledWith('run-1', 'node-run-2', 7)
  );
  expect(loadTrajectoryBody).not.toHaveBeenCalled();
});
test('retains repeated LLM executions as distinct display rows', () => {
  const base: AgentFlowTraceItem = {
    nodeId: 'same-node',
    nodeRunId: 'first',
    nodeAlias: 'LLM',
    nodeType: 'llm',
    status: 'succeeded',
    startedAt: '',
    finishedAt: '',
    durationMs: 1,
    inputPayload: { input: 'first' },
    outputPayload: {},
    errorPayload: null,
    metricsPayload: {},
    debugPayload: {}
  };
  const groups = groupTraceItemsForDisplay([
    base,
    { ...base, nodeRunId: 'second', inputPayload: { input: 'second' } }
  ]);
  expect(groups.map((group) => group.key)).toEqual(['first', 'second']);
  expect(groups.map((group) => group.item.inputPayload)).toEqual([
    { input: 'first' },
    { input: 'second' }
  ]);
});

test('shows not recorded for historical executions without supplier observations', async () => {
  const { loadTrajectory, loadTrajectoryBody } = fixture();
  loadTrajectory.mockResolvedValueOnce({
    items: [],
    next_cursor: null,
    observation_count: 0,
    persist_failed_count: 0,
    integrity: 'unavailable'
  });
  fireEvent.click(screen.getByRole('button', { name: '供应商轨迹' }));
  await waitFor(() =>
    expect(screen.getAllByText('未记录').length).toBeGreaterThan(0)
  );
  expect(screen.queryByRole('button', { name: '请求' })).toBeNull();
  expect(loadTrajectoryBody).not.toHaveBeenCalled();
});
