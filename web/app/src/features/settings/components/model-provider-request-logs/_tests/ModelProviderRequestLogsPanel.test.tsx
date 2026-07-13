import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { App } from 'antd';
import { beforeEach, vi } from 'vitest';

import { resetAuthStore, useAuthStore } from '../../../../../state/auth-store';

const requestLogsApi = vi.hoisted(() => ({
  settingsModelProviderRequestLogsQueryKey: vi.fn((filter) => [
    'request-logs',
    filter
  ]),
  fetchSettingsModelProviderRequestLogs: vi.fn(),
  deleteSettingsModelProviderRequestLogs: vi.fn(),
  clearSettingsModelProviderRequestLogsBatch: vi.fn()
}));

vi.mock('../../../api/model-providers', () => requestLogsApi);

import { ModelProviderRequestLogsPanel } from '../ModelProviderRequestLogsPanel';

function renderPanel() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: null
  });
  return render(
    <App>
      <QueryClientProvider
        client={new QueryClient({
          defaultOptions: { queries: { retry: false } }
        })}
      >
        <ModelProviderRequestLogsPanel />
      </QueryClientProvider>
    </App>
  );
}

beforeEach(() => {
  resetAuthStore();
  vi.clearAllMocks();
});

test('AC-006 renders zero output as an empty response anomaly', async () => {
  requestLogsApi.fetchSettingsModelProviderRequestLogs.mockResolvedValue({
    total_count: 1,
    page: 1,
    page_size: 20,
    items: [
      {
        attempt_id: 'attempt-1',
        flow_run_id: 'run-1',
        application_id: 'application-1',
        conversation_id: 'conversation-1',
        application_name: 'Story Agent',
        attempt_index: 1,
        provider_instance_id: 'provider-1',
        provider_instance_display_name: 'Gemini A',
        provider_code: 'gemini',
        protocol: 'gemini',
        upstream_model_id: 'gemini-3-flash',
        reasoning_effort: null,
        status: 'empty_response',
        error_code: 'provider_invalid_response',
        failed_after_first_token: false,
        input_tokens: 35629,
        output_tokens: 0,
        total_tokens: 35629,
        started_at: '2026-07-11T03:04:00Z',
        first_token_at: null,
        finished_at: '2026-07-11T03:04:05Z',
        time_to_first_token_ms: null,
        total_duration_ms: 5000
      }
    ]
  });

  renderPanel();

  expect(await screen.findByText('Story Agent')).toBeInTheDocument();
  expect(
    screen.getByRole('combobox', { name: '字段配置' })
  ).toBeInTheDocument();
  expect(
    screen
      .getByRole('textbox', { name: '应用' })
      .closest('.model-provider-request-logs-panel__filters')
  ).not.toBeNull();
  expect(
    screen
      .getByRole('button', { name: /刷\s*新/ })
      .closest('.model-provider-request-logs-panel__actions')
  ).not.toBeNull();
  expect(screen.getByText('空响应')).toBeInTheDocument();
  expect(screen.getByText('Gemini A')).toBeInTheDocument();
  expect(screen.getByText('5.00 s')).toBeInTheDocument();
  expect(screen.getByRole('link', { name: '查看对话' })).toHaveAttribute(
    'href',
    '/applications/application-1/logs?run_id=run-1'
  );
  expect(screen.getAllByText('1').some((element) => element.tagName === 'TD')).toBe(true);
});


test('AC-006 does not render a conversation link for legacy rows', async () => {
  requestLogsApi.fetchSettingsModelProviderRequestLogs.mockResolvedValue({
    total_count: 1,
    page: 1,
    page_size: 20,
    items: [
      {
        attempt_id: 'attempt-legacy',
        flow_run_id: 'run-legacy',
        application_id: null,
        conversation_id: null,
        application_name: 'Legacy App',
        attempt_index: 1,
        is_retry: false,
        retry_reason: null,
        provider_instance_id: null,
        provider_instance_display_name: null,
        provider_code: 'openai',
        protocol: 'openai_chat',
        upstream_model_id: 'gpt-5',
        reasoning_effort: null,
        status: 'succeeded',
        error_code: null,
        failed_after_first_token: false,
        input_tokens: 1,
        output_tokens: 1,
        total_tokens: 2,
        started_at: '2026-07-11T03:04:00Z',
        first_token_at: '2026-07-11T03:04:01Z',
        finished_at: '2026-07-11T03:04:02Z',
        time_to_first_token_ms: 1000,
        total_duration_ms: 2000
      }
    ]
  });

  renderPanel();

  expect(await screen.findByText('Legacy App')).toBeInTheDocument();
  expect(screen.queryByRole('link', { name: '查看对话' })).toBeNull();
});

test('AC-001 defaults to the past seven days and clears selection when the range changes', async () => {
  requestLogsApi.fetchSettingsModelProviderRequestLogs.mockResolvedValue({
    total_count: 0,
    page: 1,
    page_size: 20,
    items: []
  });
  renderPanel();

  await waitFor(() => {
    expect(requestLogsApi.fetchSettingsModelProviderRequestLogs).toHaveBeenCalled();
  });
  const initialFilter =
    requestLogsApi.fetchSettingsModelProviderRequestLogs.mock.calls[0][0];
  expect(initialFilter.started_after).toEqual(expect.any(String));
  expect(
    Date.now() - new Date(initialFilter.started_after).getTime()
  ).toBeGreaterThanOrEqual(7 * 24 * 60 * 60 * 1000 - 2_000);

  fireEvent.mouseDown(screen.getByRole('combobox', { name: '时间范围' }));
  fireEvent.click(await screen.findByText('全部时间'));
  await waitFor(() => {
    const lastFilter =
      requestLogsApi.fetchSettingsModelProviderRequestLogs.mock.calls.at(-1)?.[0];
    expect(lastFilter.started_after).toBeUndefined();
    expect(lastFilter.page).toBe(1);
  });
});

test('AC-002 deletes only selected stable attempt IDs after confirmation', async () => {
  requestLogsApi.fetchSettingsModelProviderRequestLogs.mockResolvedValue({
    total_count: 1,
    page: 1,
    page_size: 20,
    items: [
      {
        attempt_id: 'attempt-1',
        flow_run_id: 'run-1',
        application_id: null,
        conversation_id: null,
        application_name: 'Selected App',
        attempt_index: 1,
        is_retry: false,
        retry_reason: null,
        provider_instance_id: null,
        provider_instance_display_name: null,
        provider_code: 'openai',
        protocol: 'openai_chat',
        upstream_model_id: 'gpt-5',
        reasoning_effort: null,
        status: 'succeeded',
        error_code: null,
        failed_after_first_token: false,
        input_tokens: 1,
        output_tokens: 1,
        total_tokens: 2,
        started_at: '2026-07-13T03:04:00Z',
        first_token_at: null,
        finished_at: null,
        time_to_first_token_ms: null,
        total_duration_ms: null
      }
    ]
  });
  requestLogsApi.deleteSettingsModelProviderRequestLogs.mockResolvedValue({
    deleted_count: 1
  });
  renderPanel();

  expect(await screen.findByText('Selected App')).toBeInTheDocument();
  const deleteButton = screen.getByRole('button', { name: '删除选中' });
  expect(deleteButton).toBeDisabled();
  fireEvent.click(screen.getByRole('checkbox', { name: '选择请求日志' }));
  expect(deleteButton).toBeEnabled();
  fireEvent.click(deleteButton);
  fireEvent.click(await screen.findByRole('button', { name: '确认删除' }));

  await waitFor(() => {
    expect(requestLogsApi.deleteSettingsModelProviderRequestLogs).toHaveBeenCalledWith(
      { attempt_ids: ['attempt-1'] },
      'csrf-123'
    );
  });
});

test('AC-004 cancellation does not clear filtered request logs', async () => {
  requestLogsApi.fetchSettingsModelProviderRequestLogs.mockResolvedValue({
    total_count: 0,
    page: 1,
    page_size: 20,
    items: []
  });
  renderPanel();

  fireEvent.click(await screen.findByRole('button', { name: '清空日志' }));
  expect(
    await screen.findByText('将清空当前工作区的全部请求日志，不受当前筛选影响。')
  ).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: /取\s*消/ }));
  expect(
    requestLogsApi.clearSettingsModelProviderRequestLogsBatch
  ).not.toHaveBeenCalled();
});

test('AC-007 stops on a failed clear batch and retries with the opaque continuation token', async () => {
  requestLogsApi.fetchSettingsModelProviderRequestLogs.mockResolvedValue({
    total_count: 0,
    page: 1,
    page_size: 20,
    items: []
  });
  requestLogsApi.clearSettingsModelProviderRequestLogsBatch
    .mockResolvedValueOnce({
      deleted_count: 500,
      has_more: true,
      continuation_token: 'opaque-signed-token'
    })
    .mockRejectedValueOnce(new Error('network'))
    .mockResolvedValueOnce({
      deleted_count: 1,
      has_more: false,
      continuation_token: 'opaque-signed-token'
    });
  renderPanel();

  fireEvent.click(await screen.findByRole('button', { name: '清空日志' }));
  fireEvent.click(await screen.findByRole('button', { name: '确认清空' }));
  expect(await screen.findByText('已删除 500 条，清理已停止')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '重试清理' }));

  await waitFor(() => {
    expect(
      requestLogsApi.clearSettingsModelProviderRequestLogsBatch
    ).toHaveBeenLastCalledWith(
      { continuation_token: 'opaque-signed-token' },
      'csrf-123'
    );
  });
  expect(await screen.findByText('已删除 501 条')).toBeInTheDocument();
});
