import { describe, expect, test, vi } from 'vitest';

import type { JsBlockHostInterfaceEffect } from '@1flowbase/page-runtime';
import {
  createFrontstageJsBlockCapabilityHandlers,
  type FrontstageJsBlockCapabilityClient
} from '../../lib/js-block-capability-handlers';

function createClient(): FrontstageJsBlockCapabilityClient {
  return {
    dispatchFrontstageCallable: vi.fn().mockResolvedValue({ items: [] }),
    issueFrontstageCallableWriteGrant: vi.fn().mockResolvedValue({
      grant_token: 'grant-1',
      expires_at: '2026-07-20T00:00:00Z'
    })
  };
}

describe('createFrontstageJsBlockCapabilityHandlers', () => {
  test('AC-004 resolves the local alias before dispatching the registered operation', async () => {
    const client = createClient();
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      baseUrl: 'http://api.test',
      client,
      resolveBinding: (_requestId, alias) =>
        alias === 'listConversations'
          ? {
              blockId: 'block-1',
              binding: {
                alias,
                operation_id: 'list_application_conversations_records',
                schema_digest: 'digest-1',
                scope: 'frontstage_page_tab',
                risk_level: 'low'
              }
            }
          : null
    });
    const effect: JsBlockHostInterfaceEffect = {
      type: 'interface',
      requestId: 'restricted-block:block-1:code-1',
      effectId: 'effect-1',
      bindingAlias: 'listConversations',
      request: { query: { page: 1 } }
    };

    await expect(handlers.interface(effect)).resolves.toEqual({ items: [] });
    expect(client.dispatchFrontstageCallable).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'tab-1',
      {
        block_id: 'block-1',
        binding_alias: 'listConversations',
        schema_digest: 'digest-1',
        run_id: 'restricted-block:block-1:code-1',
        draft_hash: 'runtime',
        request: { query: { page: 1 } }
      },
      'csrf-1',
      'http://api.test'
    );
  });

  test('fails closed when the source block did not bind the alias', () => {
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      resolveBinding: () => null
    });
    expect(() =>
      handlers.interface({
        type: 'interface',
        requestId: 'restricted-block:block-1:code-1',
        effectId: 'effect-1',
        bindingAlias: 'unbound'
      })
    ).toThrow('Interface binding is not registered: unbound.');
  });

  test('uses a server-issued grant once for the exact high-risk draft binding', async () => {
    const client = createClient();
    const binding = {
      alias: 'savePage',
      operation_id: 'save_frontstage_tab_document',
      schema_digest: 'digest-2',
      scope: 'frontstage_page_tab',
      risk_level: 'high'
    };
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      baseUrl: 'http://api.test',
      client,
      resolveBinding: () => ({ blockId: 'block-1', binding })
    });

    await handlers.prepareDraftRun({
      blockId: 'block-1',
      runId: 'run-1',
      draftHash: 'draft-1',
      bindings: [binding]
    });
    await handlers.interface({
      type: 'interface',
      requestId: 'run-1',
      effectId: 'effect-1',
      bindingAlias: 'savePage',
      request: { body: { payload: {} } }
    });

    expect(client.issueFrontstageCallableWriteGrant).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'tab-1',
      {
        block_id: 'block-1',
        binding_alias: 'savePage',
        schema_digest: 'digest-2',
        run_id: 'run-1',
        draft_hash: 'draft-1'
      },
      'csrf-1',
      'http://api.test'
    );
    expect(client.dispatchFrontstageCallable).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'tab-1',
      expect.objectContaining({
        block_id: 'block-1',
        binding_alias: 'savePage',
        run_id: 'run-1',
        draft_hash: 'draft-1',
        write_grant: 'grant-1'
      }),
      'csrf-1',
      'http://api.test'
    );
  });
});
