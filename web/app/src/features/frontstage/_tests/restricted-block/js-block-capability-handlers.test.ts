import { describe, expect, test, vi } from 'vitest';

import type {
  JsBlockHostActionEffect,
  JsBlockHostDataEffect
} from '@1flowbase/page-runtime';

import {
  createFrontstageJsBlockCapabilityHandlers,
  type FrontstageJsBlockCapabilityClient
} from '../../lib/js-block-capability-handlers';

function createClient(): FrontstageJsBlockCapabilityClient {
  return {
    dispatchFrontstageQuery: vi.fn().mockResolvedValue({ items: [] }),
    dispatchFrontstageAction: vi.fn().mockResolvedValue({ saved: true })
  };
}

describe('createFrontstageJsBlockCapabilityHandlers', () => {
  test('AC-010 forwards only queryId and params to the bound page-tab dispatch endpoint', async () => {
    const client = createClient();
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      baseUrl: 'http://api.test',
      client
    });
    const effect: JsBlockHostDataEffect = {
      type: 'data',
      requestId: 'request-1',
      effectId: 'effect-1',
      queryId: 'frontstage.page_tab.get',
      params: { model: 'users', url: 'https://forbidden.example' }
    };

    await expect(handlers.data(effect)).resolves.toEqual({ items: [] });
    expect(client.dispatchFrontstageQuery).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'tab-1',
      {
        query_id: 'frontstage.page_tab.get',
        params: { model: 'users', url: 'https://forbidden.example' }
      },
      'http://api.test'
    );
  });

  test('AC-010 forwards actionId and params with csrf to the bound page-tab dispatch endpoint', async () => {
    const client = createClient();
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      baseUrl: 'http://api.test',
      client
    });
    const effect: JsBlockHostActionEffect = {
      type: 'action',
      requestId: 'request-1',
      effectId: 'effect-1',
      actionId: 'frontstage.page_tab.document.save',
      payload: { schema: {}, root: {} }
    };

    await expect(handlers.action(effect)).resolves.toEqual({ saved: true });
    expect(client.dispatchFrontstageAction).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'tab-1',
      {
        action_id: 'frontstage.page_tab.document.save',
        params: { schema: {}, root: {} }
      },
      'csrf-1',
      'http://api.test'
    );
  });

  test('rejects action dispatch without csrf before calling the api client', async () => {
    const client = createClient();
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      client
    });

    expect(() =>
      handlers.action({
        type: 'action',
        requestId: 'request-1',
        effectId: 'effect-1',
        actionId: 'frontstage.page_tab.document.save'
      })
    ).toThrow('JS Block action capability requires csrfToken.');
    expect(client.dispatchFrontstageAction).not.toHaveBeenCalled();
  });
});
