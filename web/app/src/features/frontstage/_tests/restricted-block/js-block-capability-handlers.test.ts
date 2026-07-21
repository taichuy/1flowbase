import { describe, expect, test, vi } from 'vitest';

import type { JsBlockHostInterfaceEffect } from '@1flowbase/page-runtime';
import {
  createFrontstageJsBlockCapabilityHandlers,
  type FrontstageJsBlockCapabilityClient
} from '../../lib/js-block-capability-handlers';

function createClient(): FrontstageJsBlockCapabilityClient {
  return {
    dispatchFrontstageCallable: vi.fn().mockResolvedValue({ items: [] }),
    dispatchFrontstageCallableBinary: vi.fn().mockResolvedValue({
      bytes: new Uint8Array([1]),
      file_name: 'download.bin',
      content_type: 'application/octet-stream'
    }),
    dispatchFrontstageCallableStream: vi.fn().mockResolvedValue({
      cancel: vi.fn(),
      async *[Symbol.asyncIterator]() {
        yield { progress: 1 };
      }
    }),
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
                risk_level: 'low',
                request_media_type: null,
                response_media_type: 'application/json'
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
      risk_level: 'high',
      request_media_type: 'application/json',
      response_media_type: 'application/json'
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

  test('opens, pulls, and cancels an SSE binding within the owning run', async () => {
    const client = createClient();
    const binding = {
      alias: 'watchRun',
      operation_id: 'stream_application_run_events',
      schema_digest: 'digest-stream',
      scope: 'frontstage_page_tab',
      risk_level: 'low',
      request_media_type: 'application/json',
      response_media_type: 'text/event-stream'
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
    const opened = (await handlers.interface({
      type: 'interface',
      requestId: 'run-1',
      effectId: 'effect-open',
      bindingAlias: 'watchRun',
      operation: 'stream_open'
    })) as { stream_id: string };
    await expect(
      handlers.interface({
        type: 'interface',
        requestId: 'run-1',
        effectId: 'effect-next',
        bindingAlias: 'watchRun',
        operation: 'stream_next',
        streamId: opened.stream_id
      })
    ).resolves.toEqual({ done: false, value: { progress: 1 } });
    await expect(
      handlers.interface({
        type: 'interface',
        requestId: 'run-1',
        effectId: 'effect-cancel',
        bindingAlias: 'watchRun',
        operation: 'stream_cancel',
        streamId: opened.stream_id
      })
    ).resolves.toBeUndefined();
  });
});
