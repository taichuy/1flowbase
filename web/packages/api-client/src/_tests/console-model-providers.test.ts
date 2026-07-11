import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';
import { listConsoleModelProviderRequestLogs } from '../console-model-providers';

describe('console model provider request logs', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('AC-007 sends request log filters and pagination', async () => {
    await expect(
      listConsoleModelProviderRequestLogs({
        application_id: 'app-1',
        provider_instance_id: 'provider-1',
        model_id: 'gemini-3-flash',
        status: 'empty_response',
        zero_output_only: true,
        page: 2,
        page_size: 20
      })
    ).resolves.toMatchObject({
      path: '/api/console/model-providers/request-logs?application_id=app-1&provider_instance_id=provider-1&model_id=gemini-3-flash&status=empty_response&zero_output_only=true&page=2&page_size=20'
    });
  });
});
