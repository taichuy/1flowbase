import { describe, expect, test, vi } from 'vitest';

import type { BlockHostInterfaceEffect } from '@1flowbase/page-runtime';
import {
  createFrontstageJsBlockCapabilityHandlers,
  type FrontstageJsBlockCapabilityClient
} from '../../lib/js-block-capability-handlers';

function createClient(): FrontstageJsBlockCapabilityClient {
  return {
    dispatchFrontstageCallable: vi.fn().mockResolvedValue({ items: [] }),
    dispatchFrontstageCallableStream: vi.fn().mockResolvedValue({
      cancel: vi.fn(),
      async *[Symbol.asyncIterator]() {
        yield { progress: 1 };
      }
    })
  };
}

function effect(
  overrides: Partial<BlockHostInterfaceEffect> = {}
): BlockHostInterfaceEffect {
  return {
    type: 'interface',
    requestId: 'run-1',
    effectId: 'effect-1',
    method: 'GET',
    path: '/api/console/test',
    request: { query: { page: 1 } },
    ...overrides
  };
}

describe('createFrontstageJsBlockCapabilityHandlers', () => {
  test('AC-020 dispatches the source route without resolving a binding', async () => {
    const client = createClient();
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      baseUrl: 'http://api.test',
      client,
      resolveBlockId: () => 'block-1'
    });

    await expect(handlers.interface(effect())).resolves.toEqual({ items: [] });
    expect(client.dispatchFrontstageCallable).toHaveBeenCalledWith(
      'page-1',
      'tab-1',
      {
        block_id: 'block-1',
        method: 'GET',
        path: '/api/console/test',
        request: { query: { page: 1 } }
      },
      'csrf-1',
      'http://api.test'
    );
  });

  test('AC-001 dispatches a draft write immediately with the current user session and no grant retry', async () => {
    const client = createClient();
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      baseUrl: 'http://api.test',
      client,
      resolveBlockId: () => 'block-7'
    });
    await handlers.prepareDraftRun({
      blockId: 'block-7',
      runId: 'draft:block-7:1'
    });
    const writeEffect = effect({
      requestId: 'draft:block-7:1',
      method: 'POST',
      path: '/api/console/records',
      request: { body: { title: 'Saved runtime' } }
    });

    await expect(handlers.interface(writeEffect)).resolves.toEqual({ items: [] });
    expect(client.dispatchFrontstageCallable).toHaveBeenCalledTimes(1);
    expect(client.dispatchFrontstageCallable).toHaveBeenCalledWith(
      'page-1',
      'tab-1',
      {
        block_id: 'block-7',
        method: 'POST',
        path: '/api/console/records',
        request: { body: { title: 'Saved runtime' } }
      },
      'csrf-1',
      'http://api.test'
    );
  });

  test('AC-002 surfaces a backend write denial without confirmation or a retry', async () => {
    const client = createClient();
    const denied = new Error('Route ACL denied this write.');
    vi.mocked(client.dispatchFrontstageCallable).mockRejectedValueOnce(denied);
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      client,
      resolveBlockId: () => 'block-1'
    });

    await expect(
      handlers.interface(effect({ method: 'DELETE' }))
    ).rejects.toBe(denied);
    expect(client.dispatchFrontstageCallable).toHaveBeenCalledTimes(1);
  });

  test('fails closed when the source block is not registered for the run', async () => {
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      resolveBlockId: () => null
    });

    await expect(handlers.interface(effect())).rejects.toThrow(
      'Interface source block is not registered.'
    );
  });

  test('AC-001 uses the prepared draft identity only to bind the source block', async () => {
    const client = createClient();
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      baseUrl: 'http://api.test',
      client,
      resolveBlockId: () => 'block-1'
    });
    await handlers.prepareDraftRun({
      blockId: 'block-1',
      runId: 'run-1'
    });
    const writeEffect = effect({
      method: 'PUT',
      path: '/api/console/frontstage/pages/{page_id}/tabs/{tab_id}/document',
      request: { body: { payload: {} } }
    });

    await expect(handlers.interface(writeEffect)).resolves.toEqual({ items: [] });
    expect(client.dispatchFrontstageCallable).toHaveBeenLastCalledWith(
      'page-1',
      'tab-1',
      {
        block_id: 'block-1',
        method: 'PUT',
        path: '/api/console/frontstage/pages/{page_id}/tabs/{tab_id}/document',
        request: { body: { payload: {} } }
      },
      'csrf-1',
      'http://api.test'
    );
  });

  test('D4-AC-002 rejects a revoked Studio run without dispatching a request', async () => {
    const client = createClient();
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      client,
      resolveBlockId: () => 'block-1'
    });
    await handlers.prepareDraftRun({
      blockId: 'block-1',
      runId: 'run-1'
    });
    handlers.revokeDraftRun('run-1');

    await expect(
      handlers.interface(effect({ method: 'POST' }))
    ).rejects.toThrow('revoked');
    expect(client.dispatchFrontstageCallable).not.toHaveBeenCalled();
  });

  test('opens, pulls, and cancels an SSE route within the owning run', async () => {
    const client = createClient();
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      baseUrl: 'http://api.test',
      client,
      resolveBlockId: () => 'block-1'
    });
    const route = {
      method: 'GET',
      path: '/api/console/test'
    };
    const opened = (await handlers.interface(
      effect({ ...route, operation: 'stream_open', request: undefined })
    )) as { stream_id: string };
    await expect(
      handlers.interface(
        effect({
          ...route,
          operation: 'stream_next',
          streamId: opened.stream_id,
          request: undefined
        })
      )
    ).resolves.toEqual({ done: false, value: { progress: 1 } });
    await expect(
      handlers.interface(
        effect({
          ...route,
          operation: 'stream_cancel',
          streamId: opened.stream_id,
          request: undefined
        })
      )
    ).resolves.toBeUndefined();
  });
});
