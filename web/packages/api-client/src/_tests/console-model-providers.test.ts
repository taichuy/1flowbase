import { describe, expect, test, vi } from 'vitest';
import * as transport from '../transport';
import {
  clearConsoleModelProviderRequestLogsBatch,
  createConsoleModelProviderInstance,
  deleteConsoleModelProviderInstance,
  deleteConsoleModelProviderRequestLogs,
  getConsoleModelProviderMainInstance,
  getConsoleModelProviderModels,
  listConsoleModelProviderCatalog,
  listConsoleModelProviderInstances,
  listConsoleModelProviderRequestLogs,
  listConsoleSettingsModelProviderOptions,
  previewConsoleModelProviderModels,
  refreshConsoleModelProviderModels,
  revealConsoleModelProviderSecret,
  updateConsoleModelProviderInstance,
  updateConsoleModelProviderMainInstance,
  validateConsoleModelProviderInstance
} from '../console-model-providers';

const csrfToken = 'csrf-token';

describe('console model provider settings route contract', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('AC-001 uses only backend-registered settings paths', async () => {
    const apiFetch = vi.mocked(transport.apiFetch);
    apiFetch.mockClear();

    await Promise.all([
      listConsoleModelProviderCatalog({ locale: 'zh_Hans' }),
      listConsoleModelProviderInstances(),
      createConsoleModelProviderInstance(
        {
          installation_id: 'installation-1',
          display_name: 'Provider 1',
          configured_models: [],
          config: {}
        },
        csrfToken
      ),
      previewConsoleModelProviderModels(
        { installation_id: 'installation-1', config: {} },
        csrfToken
      ),
      updateConsoleModelProviderInstance(
        'instance-1',
        { display_name: 'Provider 1', configured_models: [], config: {} },
        csrfToken
      ),
      getConsoleModelProviderMainInstance('provider-1'),
      updateConsoleModelProviderMainInstance(
        'provider-1',
        { auto_include_new_instances: true, expected_revision: 1 },
        csrfToken
      ),
      validateConsoleModelProviderInstance('instance-1', csrfToken),
      getConsoleModelProviderModels('instance-1'),
      refreshConsoleModelProviderModels('instance-1', csrfToken),
      revealConsoleModelProviderSecret('instance-1', 'api_key', csrfToken),
      deleteConsoleModelProviderInstance('instance-1', csrfToken),
      listConsoleSettingsModelProviderOptions(),
      listConsoleModelProviderRequestLogs(),
      deleteConsoleModelProviderRequestLogs({ attempt_ids: [] }, csrfToken),
      clearConsoleModelProviderRequestLogsBatch({}, csrfToken)
    ]);

    expect(apiFetch.mock.calls.map(([input]) => input.path)).toEqual([
      '/api/console/settings/model-providers/catalog?locale=zh_Hans',
      '/api/console/settings/model-providers/instances',
      '/api/console/settings/model-providers/instances',
      '/api/console/settings/model-providers/preview-models',
      '/api/console/settings/model-providers/instances/instance-1',
      '/api/console/settings/model-providers/providers/provider-1/main-instance',
      '/api/console/settings/model-providers/providers/provider-1/main-instance',
      '/api/console/settings/model-providers/instances/instance-1/validate',
      '/api/console/settings/model-providers/instances/instance-1/models',
      '/api/console/settings/model-providers/instances/instance-1/models/refresh',
      '/api/console/settings/model-providers/instances/instance-1/secrets/reveal',
      '/api/console/settings/model-providers/instances/instance-1',
      '/api/console/settings/model-providers/options',
      '/api/console/settings/model-providers/request-logs',
      '/api/console/settings/model-providers/request-logs',
      '/api/console/settings/model-providers/request-logs/clear'
    ]);
  });
});

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
      path: '/api/console/settings/model-providers/request-logs?application_name=Story+Agent&provider_instance_id=provider-1&model_id=gemini-3-flash&status=empty_response&zero_output_only=true&started_after=2026-07-06T00%3A00%3A00.000Z&started_before=2026-07-13T00%3A00%3A00.000Z&page=2&page_size=20'
    });
  });

  test('AC-002 sends stable attempt IDs through the selected-delete command', async () => {
    await expect(
      deleteConsoleModelProviderRequestLogs(
        { attempt_ids: ['attempt-1', 'attempt-2'] },
        'csrf-token'
      )
    ).resolves.toMatchObject({
      path: '/api/console/settings/model-providers/request-logs',
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
      path: '/api/console/settings/model-providers/request-logs/clear',
      method: 'POST',
      csrfToken: 'csrf-token',
      body: { continuation_token: 'opaque-signed-token' }
    });
  });
});
