import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { vi } from 'vitest';

const requestLogsApi = vi.hoisted(() => ({
  settingsModelProviderRequestLogsQueryKey: vi.fn(() => ['request-logs']),
  fetchSettingsModelProviderRequestLogs: vi.fn()
}));

vi.mock('../../../api/model-providers', () => requestLogsApi);

import { ModelProviderRequestLogsPanel } from '../ModelProviderRequestLogsPanel';

test('AC-006 renders zero output as an empty response anomaly', async () => {
  requestLogsApi.fetchSettingsModelProviderRequestLogs.mockResolvedValue({
    total_count: 1,
    page: 1,
    page_size: 20,
    items: [
      {
        attempt_id: 'attempt-1',
        flow_run_id: 'run-1',
        application_id: 'app-1',
        application_name: 'Story Agent',
        attempt_index: 0,
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

  render(
    <QueryClientProvider client={new QueryClient()}>
      <ModelProviderRequestLogsPanel />
    </QueryClientProvider>
  );

  expect(await screen.findByText('Story Agent')).toBeInTheDocument();
  expect(screen.getByRole('combobox', { name: '字段配置' })).toBeInTheDocument();
  expect(screen.getByText('空响应')).toBeInTheDocument();
  expect(screen.getByText('Gemini A')).toBeInTheDocument();
  expect(screen.getByText('5.00 s')).toBeInTheDocument();
});
