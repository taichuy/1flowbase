import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';
import {
  clearConsoleModelProviderRequestLogsBatch,
  deleteConsoleModelProviderRequestLogs,
  listConsoleModelProviderRequestLogs
} from '../console-model-providers';

describe('console model provider request logs', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('AC-001 sends time filters and pagination', async () => {
    await expect(
      listConsoleModelProviderRequestLogs({
        application_name: 'Story Agent',
        provider_instance_id: 'provider-1',
        model_id: 'gemini-3-flash',
        status: 'empty_response',
        zero_output_only: true,
        started_after: '2026-07-06T00:00:00.000Z',
        started_before: '2026-07-13T00:00:00.000Z',
        page: 2,
        page_size: 20
      })
    ).resolves.toMatchObject({
      path: '/api/console/model-providers/request-logs?application_name=Story+Agent&provider_instance_id=provider-1&model_id=gemini-3-flash&status=empty_response&zero_output_only=true&started_after=2026-07-06T00%3A00%3A00.000Z&started_before=2026-07-13T00%3A00%3A00.000Z&page=2&page_size=20'
    });
  });

  test('AC-002 sends stable attempt IDs through the selected-delete command', async () => {
    await expect(
      deleteConsoleModelProviderRequestLogs(
        { attempt_ids: ['attempt-1', 'attempt-2'] },
        'csrf-token'
      )
    ).resolves.toMatchObject({
      path: '/api/console/model-providers/request-logs',
      method: 'DELETE',
      csrfToken: 'csrf-token',
      body: { attempt_ids: ['attempt-1', 'attempt-2'] }
    });
  });

  test('AC-005/AC-007 reuses only the opaque server continuation token', async () => {
    await expect(
      clearConsoleModelProviderRequestLogsBatch(
        { continuation_token: 'opaque-signed-token' },
        'csrf-token'
      )
    ).resolves.toMatchObject({
      path: '/api/console/model-providers/request-logs/clear',
      method: 'POST',
      csrfToken: 'csrf-token',
      body: { continuation_token: 'opaque-signed-token' }
    });
  });
});
