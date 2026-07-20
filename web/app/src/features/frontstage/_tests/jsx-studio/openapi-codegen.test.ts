import { describe, expect, test } from 'vitest';

import type { ConsoleFrontstageCallableInterface } from '@1flowbase/api-client';

import { generateFrontstageCallableSource } from '../../lib/jsx-studio/openapi-codegen';

const operation: ConsoleFrontstageCallableInterface = {
  operation_id: 'list_application_conversations_records',
  method: 'GET',
  path: '/api/runtime/models/application_conversations/list',
  name: 'List conversations',
  description: 'List conversations',
  parameters: [],
  request_schema: {
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
  response_schema: {
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
    const result = generateFrontstageCallableSource(
      operation,
      'listApplicationConversations'
    );

    expect(result.source).toContain('operationId=list_application_conversations_records');
    expect(result.source).toContain('filter?: string;');
    expect(result.source).not.toContain('ApplicationConversationFilterValue');
    expect(result.source).toContain('interface ListApplicationConversationsResponseItem');
    expect(result.source).toContain('items: ListApplicationConversationsResponseItem[];');
    expect(result.source).toContain('async function listApplicationConversations(');
    expect(result.source).toContain("ctx.interfaces.call<ListApplicationConversationsResponse>");
    expect(result.source).toContain("'listApplicationConversations'");
    expect(result.source).not.toContain('function main');
  });

  test('rejects catalog entries that are visible but not bindable', () => {
    expect(() =>
      generateFrontstageCallableSource(
        { ...operation, bindable: false, disabled_reason: 'write_requires_run_authorization' },
        'savePage'
      )
    ).toThrow('write_requires_run_authorization');
  });
});
