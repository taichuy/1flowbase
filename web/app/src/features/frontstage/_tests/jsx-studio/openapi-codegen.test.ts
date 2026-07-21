import { describe, expect, test } from 'vitest';

import type { ConsoleFrontstageInterfaceCapability } from '@1flowbase/api-client';
import { validateJsBlockSource } from '@1flowbase/page-runtime';

import { generateFrontstageInterfaceSource } from '../../lib/jsx-studio/openapi-codegen';

const operation: ConsoleFrontstageInterfaceCapability = {
  interface_id: 'list_application_conversations_records',
  method: 'GET',
  path: '/api/runtime/models/application_conversations/list',
  name: 'List conversations',
  short_description: 'List conversations',
  parameter_schema: {
    type: 'object',
    properties: {
      query: {
        type: 'object',
        properties: {
          filter: { type: 'string' },
          page: { type: 'integer' },
          page_size: { type: 'integer' }
        }
      }
    }
  },
  result_schema: {
    type: 'object',
    required: ['items', 'total'],
    properties: {
      items: {
        type: 'array',
        items: {
          type: 'object',
          required: ['id', 'application_id'],
          properties: {
            id: { type: 'string' },
            application_id: { type: 'string' },
            external_user: { type: 'string' }
          }
        }
      },
      total: { type: 'integer' }
    }
  },
  request_media_type: 'application/json',
  response_media_type: 'application/json',
  schema_digest: 'digest-1',
  adapter_id: 'runtime_data_model',
  host_injected_parameters: [],
  scope: 'frontstage_page_tab',
  risk_level: 'low',
  authorization: 'runtime_scope_grant_and_page_tab_access',
  bindable: true,
  disabled_reason: null
};

describe('Frontstage callable OpenAPI codegen', () => {
  test('AC-002/003 emits editable DTOs and a complete bound function', () => {
    const result = generateFrontstageInterfaceSource(
      operation,
      'listApplicationConversations'
    );

    expect(result.source).toContain(
      'operationId=list_application_conversations_records'
    );
    expect(result.source).toContain('filter?: string;');
    expect(result.source).not.toContain('ApplicationConversationFilterValue');
    expect(result.source).toContain(
      'interface ListApplicationConversationsResponseItem'
    );
    expect(result.source).toContain(
      'items: ListApplicationConversationsResponseItem[];'
    );
    expect(result.source).toContain(
      'async function listApplicationConversations('
    );
    expect(result.source).toContain(
      'ctx.interfaces.call<ListApplicationConversationsResponse>'
    );
    expect(result.source).toContain("'listApplicationConversations'");
    expect(result.source).not.toContain('function main');
  });

  test('rejects catalog entries that are visible but not bindable', () => {
    expect(() =>
      generateFrontstageInterfaceSource(
        {
          ...operation,
          bindable: false,
          disabled_reason: 'write_requires_run_authorization'
        },
        'savePage'
      )
    ).toThrow('write_requires_run_authorization');
  });

  test('quotes OpenAPI DTO properties so backend field names are not treated as globals', () => {
    const result = generateFrontstageInterfaceSource(
      {
        ...operation,
        result_schema: {
          type: 'object',
          required: ['document'],
          properties: {
            document: { type: 'string' },
            'page-id': { type: 'string' }
          }
        }
      },
      'getFrontstagePageDetail'
    );

    expect(result.source).toContain('"document": string;');
    expect(result.source).toContain('"page-id"?: string;');
    expect(validateJsBlockSource(result.source)).toMatchObject({ ok: true });
  });

  test('emits explicit binary envelopes and no-content results from media truth', () => {
    const upload = generateFrontstageInterfaceSource(
      {
        ...operation,
        request_media_type: 'multipart/form-data',
        response_media_type: null,
        parameter_schema: {
          type: 'object',
          required: ['body'],
          properties: {
            body: {
              type: 'object',
              required: ['file'],
              properties: {
                file: { type: 'string', format: 'binary' }
              }
            }
          }
        },
        result_schema: {}
      },
      'uploadFile'
    );
    expect(upload.source).toContain('interface UploadFileBodyFile {');
    expect(upload.source).toContain('file: UploadFileBodyFile;');
    expect(upload.source).toContain('base64: string;');
    expect(upload.source).toContain('Promise<void>');

    const download = generateFrontstageInterfaceSource(
      {
        ...operation,
        response_media_type: 'application/zip'
      },
      'exportLogs'
    );
    expect(download.source).toContain('interface ExportLogsResponse {');
    expect(download.source).toContain('bytes: Uint8Array;');
    expect(download.source).toContain('Promise<ExportLogsResponse>');
  });

  test('emits a pull-based AsyncIterable for SSE operations', () => {
    const stream = generateFrontstageInterfaceSource(
      {
        ...operation,
        response_media_type: 'text/event-stream'
      },
      'watchConversation'
    );
    expect(stream.source).toContain('function watchConversation(');
    expect(stream.source).toContain(
      '): AsyncIterable<WatchConversationResponse>'
    );
    expect(stream.source).toContain(
      'ctx.interfaces.stream<WatchConversationResponse>'
    );
  });
});
