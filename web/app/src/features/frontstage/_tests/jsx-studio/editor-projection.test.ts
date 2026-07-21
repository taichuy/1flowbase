import { describe, expect, test } from 'vitest';

import type { ConsoleFrontstageCallableInterface } from '@1flowbase/api-client';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import {
  createFrontstageJsxBindingSnippet,
  createFrontstageJsxEditorProjection
} from '../../lib/jsx-studio/editor-projection';

const callable = {
  operation_id: 'list_application_conversations_records',
  method: 'GET',
  path: '/api/runtime/models/application_conversations/list',
  request_schema: { type: 'object', properties: {} },
  response_schema: { type: 'object', properties: {} },
  request_media_type: null,
  response_media_type: 'application/json',
  schema_digest: 'digest-1',
  bindable: true,
  disabled_reason: null
} as ConsoleFrontstageCallableInterface;

const block = {
  id: 'block-1',
  interfaces: [
    {
      alias: 'listApplicationConversations',
      operation_id: callable.operation_id,
      schema_digest: callable.schema_digest,
      scope: 'frontstage_page_tab',
      risk_level: 'low',
      request_media_type: null,
      response_media_type: 'application/json'
    }
  ]
} as unknown as FrontstageBlockInstance;

describe('Frontstage JSX editor projection', () => {
  test('AC-002/005 projects persisted bindings and an editable source comment', () => {
    const projection = createFrontstageJsxEditorProjection({
      block,
      catalogEntry: null,
      callableInterfaces: [callable]
    });
    expect(projection.bindings[0].status).toBe('current');
    expect(projection.contextComment).toContain('@1flowbase-context');
    expect(projection.contextComment).toContain('listApplicationConversations');
    expect(createFrontstageJsxBindingSnippet(projection.bindings[0])).toContain(
      'async function listApplicationConversations('
    );
    expect(projection.contextComment).not.toContain('ctx.data');
    expect(projection.monacoExtraLibs).toEqual([]);
  });
});
