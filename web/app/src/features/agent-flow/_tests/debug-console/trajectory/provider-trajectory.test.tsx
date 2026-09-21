import './navigation';
import { Select } from 'antd';
import { WindowWorkspaceWindow } from '../../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
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
function fixture(inFloatingWindow = false) {
  const loadTrajectory = vi.fn().mockResolvedValue({
    items: [
      {
        event_id: 'event-1',
        event_sequence: 7,
        event_type: 'provider_semantic_step',
        created_at: '2026-09-21T01:00:00Z',
        metadata: {
          protocol: 'openai',
          transport: 'http',
          direction: 'prepared',
          kind: 'model_call',
          status: 'recorded',
          step_key: '1:model_call:call',
          flow_run_id: 'run-1',
          node_run_id: 'node-run-2',
          invocation_id: 'invocation-1',
          provider_attempt_index: 0,
          raw_sequence_start: 1,
          raw_sequence_end: 2
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
    items: [
      {
        event_id: 'raw-1',
        sequence: 1,
        body: '  { "provider": "original" }\n',
        encoding: 'utf8'
      }
    ],
    next_cursor: 1
  });
  const loader: ConversationLogTraceLoader = {
    loadTree: vi.fn(),
    loadChildren: vi.fn(),
    loadContent: vi.fn(),
    loadTrajectory,
    loadTrajectoryBody
  };
  const trajectory = (
    <ProviderTrajectory
      runId="run-1"
      nodeRunId="node-run-2"
      loader={loader}
      executionContent={<span>route / fusion execution</span>}
    />
  );
  render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      {inFloatingWindow ? (
        <>
          <Select
            aria-label="时间筛选"
            options={[{ value: 'week', label: '本周' }]}
            defaultValue="week"
          />
          <WindowWorkspaceWindow
            active
            zIndex={2400}
            testId="trajectory-parent-window"
            title="日志浮窗"
            initialRect={() => ({ left: 0, top: 0, width: 800, height: 600 })}
            dragHandleSelector=".window-heading"
            resizeLabel={() => '调整浮窗'}
            onActivate={vi.fn()}
          >
            {trajectory}
          </WindowWorkspaceWindow>
        </>
      ) : (
        trajectory
      )}
    </QueryClientProvider>
  );
  return { loadTrajectory, loadTrajectoryBody };
}
test('loads a bounded summary page only on opening and the exact original body only on selection', async () => {
  const { loadTrajectory, loadTrajectoryBody } = fixture();
  expect(loadTrajectory).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '供应商轨迹' }));
  fireEvent.click(await screen.findByRole('button', { name: '模型调用准备' }));
  await waitFor(() =>
    expect(loadTrajectoryBody).toHaveBeenCalledWith(
      'run-1',
      'node-run-2',
      'event-1',
      undefined
    )
  );
  const body = await screen.findByText(/"provider": "original"/);
  expect(body.textContent).toBe('  { "provider": "original" }\n');
  loadTrajectoryBody.mockResolvedValueOnce({
    event_id: 'event-1',
    items: [
      {
        event_id: 'raw-2',
        sequence: 2,
        body: 'next evidence',
        encoding: 'utf8'
      }
    ],
    next_cursor: null
  });
  fireEvent.click(screen.getByRole('button', { name: '加载更多原始证据' }));
  await screen.findByText('next evidence');
  expect(loadTrajectoryBody).toHaveBeenLastCalledWith(
    'run-1',
    'node-run-2',
    'event-1',
    1
  );
  expect(loadTrajectory).toHaveBeenCalledWith('run-1', 'node-run-2', undefined);
  fireEvent.click(screen.getByRole('tab', { name: '执行关联' }));
  expect(screen.getByText('route / fusion execution')).toBeTruthy();
});
test('does not fetch protocol bodies while browsing summary pages', async () => {
  const { loadTrajectory, loadTrajectoryBody } = fixture();
  fireEvent.click(screen.getByRole('button', { name: '供应商轨迹' }));
  await screen.findByRole('button', { name: '模型调用准备' });
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
    integrity: 'not_recorded'
  });
  fireEvent.click(screen.getByRole('button', { name: '供应商轨迹' }));
  await waitFor(() =>
    expect(screen.getAllByText('未记录').length).toBeGreaterThan(0)
  );
  expect(screen.queryByRole('button', { name: '模型调用准备' })).toBeNull();
  expect(loadTrajectoryBody).not.toHaveBeenCalled();
});

test('keeps its accessible title beside page controls and opens above the containing floating window', async () => {
  fixture(true);
  fireEvent.click(screen.getByRole('button', { name: '供应商轨迹' }));
  const dialog = await screen.findByRole('dialog', { name: '供应商轨迹' });
  const title = document.getElementById(
    dialog.getAttribute('aria-labelledby')!
  );
  expect(title).toHaveTextContent('供应商轨迹');
  expect(screen.getByTestId('trajectory-parent-window')).toHaveStyle({
    zIndex: '2400'
  });
  expect(dialog.closest('.ant-modal-wrap')).toHaveStyle({ zIndex: '2401' });
});
