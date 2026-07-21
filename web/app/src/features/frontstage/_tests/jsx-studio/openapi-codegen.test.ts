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
  test('AC-020/021 emits one callable with inline DTOs and a canonical HTTP route', () => {
    const result = generateFrontstageInterfaceSource(
      operation,
      'listApplicationConversations'
    );

    expect(result.source).not.toContain('operationId');
    expect(result.source).not.toContain('schemaDigest');
    expect(result.source).not.toContain('interface ListApplication');
    expect(result.source).toContain('filter?: string;');
    expect(result.source).not.toContain('ApplicationConversationFilterValue');
    expect(result.source).toContain('const listApplicationConversations = (');
    expect(result.source).toContain('ctx.api.get(');
    expect(result.source).toContain(
      "'/api/runtime/models/application_conversations/list'"
    );
    expect(result.source).toContain('items: {');
    expect(result.source).toContain('}[];');
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

  test('flattens path parameters and keeps the request body inside one callable', () => {
    const result = generateFrontstageInterfaceSource(
      {
        ...operation,
        method: 'PUT',
        path: '/api/console/applications/{application_id}',
        parameter_schema: {
          type: 'object',
          required: ['path', 'body'],
          properties: {
            path: {
              type: 'object',
              required: ['application_id'],
              properties: { application_id: { type: 'string' } }
            },
            body: {
              type: 'object',
              required: ['name'],
              properties: {
                name: { type: 'string' },
                description: { type: 'string' }
              }
            }
          }
        }
      },
      'updateApplication'
    );

    expect(result.source).toContain('applicationId: string');
    expect(result.source).toContain('body: {');
    expect(result.source).toContain("ctx.api.put(");
    expect(result.source).toContain(
      "'/api/console/applications/{application_id}'"
    );
    expect(result.source).toContain(
      '{ path: { application_id: applicationId }, body }'
    );
    expect(result.source).not.toContain('interface UpdateApplication');
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
    expect(upload.source).not.toContain('interface UploadFile');
    expect(upload.source).toContain('file: {');
    expect(upload.source).toContain('base64: string;');
    expect(upload.source).toContain('Promise<void>');

    const download = generateFrontstageInterfaceSource(
      {
        ...operation,
        response_media_type: 'application/zip'
      },
      'exportLogs'
    );
    expect(download.source).not.toContain('interface ExportLogsResponse');
    expect(download.source).toContain('bytes: Uint8Array;');
    expect(download.source).toContain('Promise<{');
  });

  test('emits a pull-based AsyncIterable for SSE operations', () => {
    const stream = generateFrontstageInterfaceSource(
      {
        ...operation,
        response_media_type: 'text/event-stream'
      },
      'watchConversation'
    );
    expect(stream.source).toContain('const watchConversation = (');
    expect(stream.source).toContain('): AsyncIterable<{');
    expect(stream.source).toContain('ctx.api.stream(');
    expect(stream.source).toContain("'GET'");
    expect(stream.source).toContain(
      "'/api/runtime/models/application_conversations/list'"
    );
  });
});
