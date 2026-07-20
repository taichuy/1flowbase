import { describe, expect, test, vi } from 'vitest';

import type { JsBlockHostInterfaceEffect } from '@1flowbase/page-runtime';
import {
  createFrontstageJsBlockCapabilityHandlers,
  type FrontstageJsBlockCapabilityClient
} from '../../lib/js-block-capability-handlers';

function createClient(): FrontstageJsBlockCapabilityClient {
  return {
    dispatchFrontstageCallable: vi.fn().mockResolvedValue({ items: [] })
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
      resolveOperationId: (_requestId, alias) =>
        alias === 'listConversations'
          ? 'list_application_conversations_records'
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
        operation_id: 'list_application_conversations_records',
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
      resolveOperationId: () => null
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
});
