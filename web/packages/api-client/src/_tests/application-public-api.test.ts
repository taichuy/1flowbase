import { afterEach, describe, expect, test, vi } from 'vitest';

import type {
  ConsoleApplicationApiMapping,
  ConsoleApplicationOperationBindingProjection
} from '../application-public-api';
import {
  APPLICATION_PUBLIC_RUNTIME_PATHS,
  createConsoleApplicationApiKey,
  fetchConsoleApplicationApiDocsCategoryOperations,
  fetchConsoleApplicationApiDocsCategorySpec,
  fetchConsoleApplicationApiOperationSpec,
  getConsoleWorkflowScheduleTrigger,
  getConsoleApplicationApiMapping,
  getConsoleApplicationOperationBindings,
  getConsoleApplicationApiPublication,
  listConsoleApplicationApiKeys,
  publishConsoleApplicationApiVersion,
  replaceConsoleApplicationApiMapping,
  replaceConsoleWorkflowScheduleTrigger,
  revokeConsoleApplicationApiKey,
  unpublishConsoleApplicationApiVersion,
  updateConsoleApplicationApiStatus
} from '../application-public-api';

function jsonResponse(data: unknown) {
  return new Response(JSON.stringify({ data, meta: null }), {
    status: 200,
    headers: { 'content-type': 'application/json' }
  });
}

function rawJsonResponse(data: unknown) {
  return new Response(JSON.stringify(data), {
    status: 200,
    headers: { 'content-type': 'application/json' }
  });
}

describe('application public API client', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  test('uses application-scoped console paths for key lifecycle', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(jsonResponse([]))
      .mockResolvedValueOnce(
        jsonResponse({
          id: 'key-1',
          name: 'Server key',
          token: 'sk-secret',
          token_prefix: 'sk-1234',
          creator_user_id: 'user-1',
          enabled: true,
          expires_at: null,
          last_used_at: null,
          created_at: '2026-05-09T00:00:00Z',
          updated_at: '2026-05-09T00:00:00Z'
        })
      )
      .mockResolvedValueOnce(new Response(null, { status: 204 }));

    await listConsoleApplicationApiKeys('app-1', 'http://localhost:7800');
    await createConsoleApplicationApiKey(
      'app-1',
      { name: 'Server key', expires_at: null },
      'csrf-1',
      'http://localhost:7800'
    );
    await revokeConsoleApplicationApiKey(
      'app-1',
      'key-1',
      'csrf-1',
      'http://localhost:7800'
    );

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      'http://localhost:7800/api/console/applications/app-1/api-keys',
      expect.objectContaining({ method: 'GET' })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      'http://localhost:7800/api/console/applications/app-1/api-keys',
      expect.objectContaining({
        method: 'POST',
        headers: expect.objectContaining({ 'x-csrf-token': 'csrf-1' }),
        body: JSON.stringify({ name: 'Server key', expires_at: null })
      })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      'http://localhost:7800/api/console/applications/app-1/api-keys/key-1',
      expect.objectContaining({
        method: 'DELETE',
        headers: expect.objectContaining({ 'x-csrf-token': 'csrf-1' })
      })
    );
  });

  test('uses application-scoped console paths for mapping and publication', async () => {
    const mapping: ConsoleApplicationApiMapping = {
      input: {
        query_target: 'start.query',
        model_target: null,
        inputs_target: 'start.inputs',
        history_target: 'start.history',
        attachments_target: 'start.attachments'
      },
      output: {
        answer_selector: 'answer',
        usage_selector: 'usage',
        files_selector: null,
        error_selector: 'error'
      },
      extension: {
        slug: 'ticket_webhook',
        method: 'PATCH',
        access_policy: 'user_api_key',
        response_mode: 'sync'
      }
    };
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(jsonResponse(mapping))
      .mockResolvedValueOnce(jsonResponse(mapping))
      .mockResolvedValueOnce(
        jsonResponse({ id: 'pub-1', mapping_snapshot: mapping })
      )
      .mockResolvedValueOnce(
        jsonResponse({ id: 'pub-2', mapping_snapshot: mapping })
      )
      .mockResolvedValueOnce(
        jsonResponse({ application_id: 'app-1', api_enabled: false })
      );

    await getConsoleApplicationApiMapping('app-1', 'http://localhost:7800');
    await replaceConsoleApplicationApiMapping(
      'app-1',
      mapping,
      'csrf-1',
      'http://localhost:7800'
    );
    await getConsoleApplicationApiPublication('app-1', 'http://localhost:7800');
    await publishConsoleApplicationApiVersion(
      'app-1',
      { mapping, api_enabled: true },
      'csrf-1',
      'http://localhost:7800'
    );
    await updateConsoleApplicationApiStatus(
      'app-1',
      { api_enabled: false },
      'csrf-1',
      'http://localhost:7800'
    );

    expect(fetchMock.mock.calls.map((call) => call[0])).toEqual([
      'http://localhost:7800/api/console/applications/app-1/api-mapping',
      'http://localhost:7800/api/console/applications/app-1/api-mapping',
      'http://localhost:7800/api/console/applications/app-1/api-publication',
      'http://localhost:7800/api/console/applications/app-1/api-publications',
      'http://localhost:7800/api/console/applications/app-1/api-status'
    ]);
    expect(fetchMock.mock.calls[1]?.[1]).toEqual(
      expect.objectContaining({
        method: 'PUT',
        body: JSON.stringify(mapping)
      })
    );
    expect(fetchMock.mock.calls[3]?.[1]).toEqual(
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ mapping, api_enabled: true })
      })
    );
    expect(fetchMock.mock.calls[4]?.[1]).toEqual(
      expect.objectContaining({
        method: 'PATCH',
        body: JSON.stringify({ api_enabled: false })
      })
    );
  });

  test('reads the server-owned operation binding projection without deriving capability', async () => {
    const projection: ConsoleApplicationOperationBindingProjection = {
      editable: false,
      draft: {
        operation_bindings: {
          generate: { target_node_id: 'node-draft-generate' },
          count_tokens: null,
          compact: {
            responses_compact: null,
            responses_compaction_v2: null
          }
        },
        options: [
          {
            operation: 'generate',
            targets: [
              {
                target_node_id: 'node-draft-generate',
                node_alias: 'Draft generate'
              }
            ]
          }
        ]
      },
      published: {
        publication_id: 'publication-1',
        compiled_plan_id: 'compiled-plan-1',
        bindings: [
          {
            operation: 'generate',
            target_node_id: 'node-frozen-generate',
            status: 'supported',
            target: {
              target_node_id: 'node-frozen-generate',
              node_alias: 'Frozen generate'
            },
            unsupported_reason: null
          },
          {
            operation: 'count_tokens',
            target_node_id: null,
            status: 'unbound',
            target: null,
            unsupported_reason: null
          },
          {
            operation: 'compact.responses_compact',
            target_node_id: 'node-frozen-compact',
            status: 'unsupported',
            target: null,
            unsupported_reason: 'provider_capability_unsupported'
          }
        ]
      }
    };
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(jsonResponse(projection));

    await expect(
      getConsoleApplicationOperationBindings('app-1', 'http://localhost:7800')
    ).resolves.toEqual(projection);
    expect(fetchMock).toHaveBeenCalledWith(
      'http://localhost:7800/api/console/applications/app-1/api-operation-bindings',
      expect.objectContaining({ method: 'GET' })
    );
  });

  test('unpublish issues DELETE on the active publication path', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(new Response(null, { status: 204 }));

    await unpublishConsoleApplicationApiVersion(
      'app-1',
      'csrf-1',
      'http://localhost:7800'
    );

    expect(fetchMock.mock.calls[0]?.[0]).toBe(
      'http://localhost:7800/api/console/applications/app-1/api-publication'
    );
    expect(fetchMock.mock.calls[0]?.[1]).toEqual(
      expect.objectContaining({ method: 'DELETE' })
    );
  });

  test('uses workflow schedule trigger console paths', async () => {
    const trigger = {
      id: 'trigger-1',
      workspace_id: 'workspace-1',
      application_id: 'app-1',
      enabled: true,
      cron: '0 9 * * *',
      timezone: 'UTC',
      input_payload: {},
      created_by: 'user-1',
      updated_by: 'user-1',
      created_at: '2026-06-30T09:00:00Z',
      updated_at: '2026-06-30T09:00:00Z'
    };
    const input = {
      enabled: true,
      cron: '0 9 * * *',
      timezone: 'UTC',
      input_payload: { ticket_id: 'T-1' }
    };
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(jsonResponse(null))
      .mockResolvedValueOnce(jsonResponse(trigger));

    await getConsoleWorkflowScheduleTrigger('app-1', 'http://localhost:7800');
    await replaceConsoleWorkflowScheduleTrigger(
      'app-1',
      input,
      'csrf-1',
      'http://localhost:7800'
    );

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      'http://localhost:7800/api/console/applications/app-1/workflow-schedule-trigger',
      expect.objectContaining({ method: 'GET' })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      'http://localhost:7800/api/console/applications/app-1/workflow-schedule-trigger',
      expect.objectContaining({
        method: 'PUT',
        headers: expect.objectContaining({ 'x-csrf-token': 'csrf-1' }),
        body: JSON.stringify(input)
      })
    );
  });

  test('uses application-scoped docs routes and raw OpenAPI responses', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(
        jsonResponse({ id: 'openai-compatible-api', operations: [] })
      )
      .mockResolvedValueOnce(rawJsonResponse({ openapi: '3.1.0', paths: {} }))
      .mockResolvedValueOnce(rawJsonResponse({ openapi: '3.1.0', paths: {} }));

    await fetchConsoleApplicationApiDocsCategoryOperations(
      'app-1',
      'openai-compatible-api',
      'http://localhost:7800'
    );
    await fetchConsoleApplicationApiDocsCategorySpec(
      'app-1',
      'openai-compatible-api',
      'http://localhost:7800'
    );
    await fetchConsoleApplicationApiOperationSpec(
      'app-1',
      'applicationOpenAiCreateChatCompletion',
      'http://localhost:7800'
    );

    expect(fetchMock.mock.calls.map((call) => call[0])).toEqual([
      'http://localhost:7800/api/console/applications/app-1/api-docs/categories/openai-compatible-api/operations',
      'http://localhost:7800/api/console/applications/app-1/api-docs/categories/openai-compatible-api/openapi.json',
      'http://localhost:7800/api/console/applications/app-1/api-docs/operations/applicationOpenAiCreateChatCompletion/openapi.json'
    ]);
  });

  test('passes locale through application-scoped docs routes', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(
        jsonResponse({ id: 'openai-compatible-api', operations: [] })
      )
      .mockResolvedValueOnce(rawJsonResponse({ openapi: '3.1.0', paths: {} }))
      .mockResolvedValueOnce(rawJsonResponse({ openapi: '3.1.0', paths: {} }));

    await fetchConsoleApplicationApiDocsCategoryOperations(
      'app-1',
      'openai-compatible-api',
      'http://localhost:7800',
      'zh_Hans'
    );
    await fetchConsoleApplicationApiDocsCategorySpec(
      'app-1',
      'openai-compatible-api',
      'http://localhost:7800',
      'zh_Hans'
    );
    await fetchConsoleApplicationApiOperationSpec(
      'app-1',
      'applicationOpenAiCreateChatCompletion',
      'http://localhost:7800',
      'zh_Hans'
    );

    expect(fetchMock.mock.calls.map((call) => call[0])).toEqual([
      'http://localhost:7800/api/console/applications/app-1/api-docs/categories/openai-compatible-api/operations?locale=zh_Hans',
      'http://localhost:7800/api/console/applications/app-1/api-docs/categories/openai-compatible-api/openapi.json?locale=zh_Hans',
      'http://localhost:7800/api/console/applications/app-1/api-docs/operations/applicationOpenAiCreateChatCompletion/openapi.json?locale=zh_Hans'
    ]);
  });

  test('passes pagination and search through application-scoped docs operations route', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(
        jsonResponse({ id: 'openai-compatible-api', operations: [] })
      );

    await fetchConsoleApplicationApiDocsCategoryOperations(
      'app-1',
      'openai-compatible-api',
      { offset: 20, limit: 20, q: 'chat completion' },
      'http://localhost:7800',
      'zh_Hans'
    );

    expect(fetchMock.mock.calls.map((call) => call[0])).toEqual([
      'http://localhost:7800/api/console/applications/app-1/api-docs/categories/openai-compatible-api/operations?locale=zh_Hans&offset=20&limit=20&q=chat+completion'
    ]);
  });

  test('keeps public runtime path examples application-id-free', () => {
    expect(Object.values(APPLICATION_PUBLIC_RUNTIME_PATHS)).toEqual([
      '/api/agent/v1/runs',
      '/api/agent/v1/files',
      '/v1/chat/completions',
      '/v1/messages'
    ]);
    for (const path of Object.values(APPLICATION_PUBLIC_RUNTIME_PATHS)) {
      expect(path).not.toContain('application');
    }
  });
});
