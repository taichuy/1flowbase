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
          source: 'ai_native',
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
    protocol_integrity: 'not_recorded',
    protocol_persist_failed_count: 0,
    integrity: 'complete'
  });
  const loadTrajectoryBody = vi
    .fn()
    .mockImplementation((_run, _node, _event, _cursor, view) =>
      Promise.resolve({
        source: view === 'protocol' ? 'supplier_protocol' : 'ai_native',
        evidence_scope: view === 'protocol' ? 'invocation' : 'step',
        event_id: 'event-1',
        items: [
          {
            event_id: 'raw-1',
            sequence: 1,
            body:
              view === 'protocol'
                ? '  { "provider": "original" }\n'
                : '{"model":"native-model","unknown_extension":{"vendor_fact":true}}',
            encoding: 'utf8'
          }
        ],
        next_cursor: view === 'protocol' ? 1 : null
      })
    );
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
test('loads Native details on selection and raw protocol only after explicit opening', async () => {
  const { loadTrajectory, loadTrajectoryBody } = fixture();
  expect(loadTrajectory).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '调用轨迹' }));
  fireEvent.click(await screen.findByRole('button', { name: '模型调用准备' }));
  await waitFor(() =>
    expect(loadTrajectoryBody).toHaveBeenCalledWith(
      'run-1',
      'node-run-2',
      'event-1',
      undefined,
      'semantic'
    )
  );
  await screen.findByText(/native-model/);
  expect(
    loadTrajectoryBody.mock.calls.every((call) => call[4] === 'semantic')
  ).toBe(true);
  fireEvent.click(screen.getByRole('tab', { name: '原始协议证据' }));
  await screen.findByText(
    '以下是本次调用的协议证据，可能包含多个步骤及插件内部重试。'
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
    1,
    'protocol'
  );
  expect(loadTrajectory).toHaveBeenCalledWith('run-1', 'node-run-2', undefined);
  fireEvent.click(screen.getByRole('tab', { name: '执行关联' }));
  expect(screen.getByText('route / fusion execution')).toBeTruthy();
});
test('does not fetch protocol bodies while browsing summary pages', async () => {
  const { loadTrajectory, loadTrajectoryBody } = fixture();
  fireEvent.click(screen.getByRole('button', { name: '调用轨迹' }));
  await screen.findByRole('button', { name: '模型调用准备' });
  expect(loadTrajectoryBody).not.toHaveBeenCalled();
  loadTrajectory.mockResolvedValueOnce({
    items: [],
    next_cursor: null,
    observation_count: 2,
    persist_failed_count: 0,
    protocol_integrity: 'not_recorded',
    protocol_persist_failed_count: 0,
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
    protocol_integrity: 'not_recorded',
    protocol_persist_failed_count: 0,
    integrity: 'not_recorded'
  });
  fireEvent.click(screen.getByRole('button', { name: '调用轨迹' }));
  await waitFor(() =>
    expect(screen.getAllByText('未记录').length).toBeGreaterThan(0)
  );
  expect(screen.queryByRole('button', { name: '模型调用准备' })).toBeNull();
  expect(loadTrajectoryBody).not.toHaveBeenCalled();
});

test('keeps its accessible title beside page controls and opens above the containing floating window', async () => {
  fixture(true);
  fireEvent.click(screen.getByRole('button', { name: '调用轨迹' }));
  const dialog = await screen.findByRole('dialog', { name: '调用轨迹' });
  const title = document.getElementById(
    dialog.getAttribute('aria-labelledby')!
  );
  expect(title).toHaveTextContent('调用轨迹');
  expect(screen.getByTestId('trajectory-parent-window')).toHaveStyle({
    zIndex: '2400'
  });
  expect(dialog.closest('.ant-modal-wrap')).toHaveStyle({ zIndex: '2401' });
});

test('keeps complete semantic records when raw evidence was not captured', async () => {
  const { loadTrajectoryBody } = fixture();
  fireEvent.click(screen.getByRole('button', { name: '调用轨迹' }));
  await screen.findByText('语义记录: 记录完整');
  expect(screen.getByText('原始协议: 未记录')).toBeTruthy();
  fireEvent.click(screen.getByRole('button', { name: '模型调用准备' }));
  await screen.findByText(/native-model/);
  expect(screen.getByText('AI Native 调用')).toBeTruthy();
  expect(
    loadTrajectoryBody.mock.calls.every((call) => call[4] === 'semantic')
  ).toBe(true);
});

test('labels historical supplier projections and keeps raw failure separate', async () => {
  const { loadTrajectory, loadTrajectoryBody } = fixture();
  const page = await loadTrajectory.getMockImplementation()!();
  page.items[0].metadata.source = 'supplier_protocol';
  page.protocol_integrity = 'incomplete';
  loadTrajectory.mockResolvedValue(page);
  fireEvent.click(screen.getByRole('button', { name: '调用轨迹' }));
  await screen.findByText('语义记录: 记录完整');
  expect(screen.getByText('原始协议: 记录不完整')).toBeTruthy();
  fireEvent.click(screen.getByRole('button', { name: '模型调用准备' }));
  await screen.findByText('供应商协议投影');
  expect(
    loadTrajectoryBody.mock.calls.every((call) => call[4] === 'semantic')
  ).toBe(true);
});
