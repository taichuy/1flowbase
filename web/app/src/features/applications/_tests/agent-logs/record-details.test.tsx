import '../../../../test/fixtures/rc-util-unique-ids';
import type { ReactNode } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { App } from 'antd';
import { beforeEach, expect, test, vi } from 'vitest';
import { appI18n } from '../../../../shared/i18n/app-i18n';
import { ApplicationLogsWorkspace } from '../../components/logs/workspace/ApplicationLogsWorkspace';
import { ApplicationRunDetailPanel } from '../../components/logs/ApplicationRunDetailPanel';
import { ConversationLogPanel } from '../../../agent-flow/components/debug-console/ConversationLogPanel';
import {
  buildApplicationRunTraceMessage,
  createApplicationLogTraceLoader
} from '../../components/logs/application-log-trace-loader';

const api = vi.hoisted(() => ({
  fetchApplicationLogRecord: vi.fn(),
  fetchApplicationLogRecordClientTrajectory: vi.fn(),
  fetchApplicationLogRecordClientTrajectorySection: vi.fn()
}));
const native = vi.hoisted(() => ({
  fetchApplicationRunConversationMessages: vi.fn(),
  fetchApplicationRuns: vi.fn()
}));
vi.mock('../../api/trajectory', async (original) => ({
  ...(await original<typeof import('../../api/trajectory')>()),
  ...api
}));
vi.mock('../../api/runtime', async (original) => ({
  ...(await original<typeof import('../../api/runtime')>()),
  ...native
}));
const record = {
  record_id: 'record-1',
  source_kind: 'imported',
  source_client: 'codex',
  source_session_id: 'session-1',
  source_task_id: 'turn-1',
  native_run_id: null,
  title: 'Question',
  outcome: 'final_answer_observed',
  messages: [
    { role: 'system', content: 'System context', sequence: 1 },
    { role: 'user', content: 'Question', sequence: 2 },
    { role: 'assistant', content: 'Final answer', sequence: 4 }
  ],
  total_tokens: 0,
  input_tokens: 0,
  output_tokens: 0,
  input_cache_hit_tokens: 0,
  cost_breakdown: { total_cost: '0' },
  available_views: ['conversation', 'client_trajectory']
};
function renderWithClient(element: ReactNode) {
  return render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { queries: { retry: false } } })
      }
    >
      <App>{element}</App>
    </QueryClientProvider>
  );
}
beforeEach(async () => {
  vi.clearAllMocks();
  await appI18n.changeLanguage('zh_Hans');
  api.fetchApplicationLogRecord.mockResolvedValue(record);
  api.fetchApplicationLogRecordClientTrajectory.mockResolvedValue({
    items: [],
    next_cursor: null,
    integrity: 'complete'
  });
});
test('existing message UI renders backend system/question/final projection and never reads native conversation', async () => {
  renderWithClient(
    <ApplicationRunDetailPanel
      applicationId="app-1"
      runId="record-1"
      recordId="record-1"
      traceLoader={createApplicationLogTraceLoader('app-1', 'imported')}
      onClose={vi.fn()}
    />
  );
  expect(await screen.findByText('Final answer')).toBeInTheDocument();
  expect(screen.getByText('Question')).toBeInTheDocument();
  expect(screen.getByText('System context')).toBeInTheDocument();
  expect(native.fetchApplicationRunConversationMessages).not.toHaveBeenCalled();
  expect(api.fetchApplicationLogRecordClientTrajectory).not.toHaveBeenCalled();
  expect(screen.queryByText('Intermediate reasoning')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  await waitFor(() =>
    expect(api.fetchApplicationLogRecordClientTrajectory).toHaveBeenCalledWith(
      'app-1',
      'record-1',
      undefined,
      undefined
    )
  );
  expect(screen.queryByText('内部调用')).not.toBeInTheDocument();
});
test('original detail panel shows zero fee with existing formatting and uses record overview', async () => {
  renderWithClient(
    <ConversationLogPanel
      message={buildApplicationRunTraceMessage('record-1')}
      onClose={vi.fn()}
      overviewLoader={{
        loadRecordOverview: () =>
          api.fetchApplicationLogRecord('app-1', 'record-1')
      }}
    />
  );
  expect(await screen.findByText('0 $')).toBeInTheDocument();
  expect(screen.getByText('codex')).toBeInTheDocument();
});
test('empty record and API error remain observable in existing detail surface', async () => {
  api.fetchApplicationLogRecord.mockResolvedValue({ ...record, messages: [] });
  const view = renderWithClient(
    <ApplicationRunDetailPanel
      applicationId="app-1"
      runId="record-1"
      recordId="record-1"
      onClose={vi.fn()}
    />
  );
  await waitFor(() =>
    expect(view.container.querySelector('.ant-empty')).not.toBeNull()
  );
  view.unmount();
  api.fetchApplicationLogRecord.mockRejectedValue(new Error('unavailable'));
  renderWithClient(
    <ApplicationRunDetailPanel
      applicationId="app-1"
      runId="record-error"
      recordId="record-error"
      onClose={vi.fn()}
    />
  );
  expect(await screen.findByRole('alert')).toBeInTheDocument();
});

test('record loading stays in the original detail panel without fetching native facts', async () => {
  api.fetchApplicationLogRecord.mockImplementation(() => new Promise(() => {}));
  const view = renderWithClient(
    <ApplicationRunDetailPanel
      applicationId="app-1"
      runId="pending-record"
      recordId="pending-record"
      onClose={vi.fn()}
    />
  );
  await waitFor(() =>
    expect(view.container.querySelector('.ant-spin')).not.toBeNull()
  );
  expect(native.fetchApplicationRunConversationMessages).not.toHaveBeenCalled();
});

test('first-layer existing log table opens the original record conversation and third-layer trajectory', async () => {
  native.fetchApplicationRuns.mockResolvedValue({
    items: [
      {
        id: 'record-1',
        application_id: 'app-1',
        source_kind: 'imported',
        source_client: 'codex',
        title: 'Question',
        status: 'succeeded',
        total_cost: 0,
        total_tokens: 0,
        input_tokens: 0,
        output_tokens: 0,
        started_at: '2026-10-07T08:00:00Z',
        requested_model_id: null,
        reasoning_effort: null,
        log_conversation_id: null,
        invocation_count: 0,
        compaction_count: 0,
        parent_run_id: null,
        outcome: 'final_answer_observed'
      }
    ],
    total: 1,
    page: 1,
    page_size: 20
  });
  window.history.replaceState(null, '', '/applications/app-1/logs');
  renderWithClient(
    <ApplicationLogsWorkspace
      applicationId="app-1"
      applicationType="agent_logs"
    />
  );
  expect(await screen.findByText('0 $')).toBeInTheDocument();
  expect(screen.getByText('codex')).toBeInTheDocument();
  expect(
    screen
      .getByTestId('application-logs-list')
      .querySelector('.application-runs-table')
  ).not.toBeNull();
  fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));
  expect(await screen.findByText('Final answer')).toBeInTheDocument();
  expect(
    screen.getByTestId('application-logs-floating-run-detail')
  ).toBeInTheDocument();
  expect(native.fetchApplicationRunConversationMessages).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  expect(await screen.findByTestId('trajectory-window')).toBeInTheDocument();
});
