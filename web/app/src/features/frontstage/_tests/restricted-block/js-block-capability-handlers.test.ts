import { describe, expect, test, vi } from 'vitest';

import { ApiClientError } from '@1flowbase/api-client';
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
    }),
    issueFrontstageCallableWriteGrant: vi.fn().mockResolvedValue({
      grant_token: 'grant-1',
      expires_at: '2026-07-20T00:00:00Z'
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
      'workspace-1',
      'page-1',
      'tab-1',
      {
        block_id: 'block-1',
        method: 'GET',
        path: '/api/console/test',
        run_id: 'run-1',
        draft_hash: 'runtime:run-1',
        request: { query: { page: 1 } }
      },
      'csrf-1',
      'http://api.test'
    );
  });

  test('B-P1 confirms a saved runtime write and binds its one-time grant to the complete call identity', async () => {
    const client = createClient();
    vi.mocked(client.dispatchFrontstageCallable)
      .mockRejectedValueOnce(writeGrantRequired())
      .mockResolvedValueOnce({ created: true });
    const confirmRuntimeWrite = vi.fn().mockResolvedValue(true);
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      baseUrl: 'http://api.test',
      client,
      confirmRuntimeWrite,
      resolveBlockId: () => 'block-7'
    });
    const writeEffect = effect({
      requestId: 'native:block-7:block-7:portal-3',
      method: 'POST',
      path: '/api/console/records',
      request: { body: { title: 'Saved runtime' } }
    });

    await expect(handlers.interface(writeEffect)).resolves.toEqual({
      created: true
    });

    expect(confirmRuntimeWrite).toHaveBeenCalledWith({
      blockId: 'block-7',
      method: 'POST',
      path: '/api/console/records',
      requestId: 'native:block-7:block-7:portal-3'
    });
    expect(client.issueFrontstageCallableWriteGrant).toHaveBeenCalledWith(
      'workspace-1',
      'page-1',
      'tab-1',
      {
        block_id: 'block-7',
        method: 'POST',
        path: '/api/console/records',
        run_id: 'native:block-7:block-7:portal-3',
        draft_hash: 'runtime:native:block-7:block-7:portal-3'
      },
      'csrf-1',
      'http://api.test'
    );
    expect(client.dispatchFrontstageCallable).toHaveBeenLastCalledWith(
      'workspace-1',
      'page-1',
      'tab-1',
      expect.objectContaining({
        block_id: 'block-7',
        method: 'POST',
        path: '/api/console/records',
        run_id: 'native:block-7:block-7:portal-3',
        draft_hash: 'runtime:native:block-7:block-7:portal-3',
        write_grant: 'grant-1'
      }),
      'csrf-1',
      'http://api.test'
    );
  });

  test('B-P1 does not grant or retry when a saved runtime write is cancelled', async () => {
    const client = createClient();
    vi.mocked(client.dispatchFrontstageCallable).mockRejectedValueOnce(
      writeGrantRequired()
    );
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      client,
      confirmRuntimeWrite: vi.fn().mockResolvedValue(false),
      resolveBlockId: () => 'block-1'
    });

    await expect(
      handlers.interface(effect({ method: 'DELETE' }))
    ).rejects.toThrow('Write interface call was cancelled.');
    expect(client.dispatchFrontstageCallable).toHaveBeenCalledTimes(1);
    expect(client.issueFrontstageCallableWriteGrant).not.toHaveBeenCalled();
  });

  test('B-P1 requests a fresh grant and confirmation for every saved runtime write', async () => {
    const client = createClient();
    vi.mocked(client.dispatchFrontstageCallable)
      .mockRejectedValueOnce(writeGrantRequired())
      .mockResolvedValueOnce({ saved: 1 })
      .mockRejectedValueOnce(writeGrantRequired())
      .mockResolvedValueOnce({ saved: 2 });
    vi.mocked(client.issueFrontstageCallableWriteGrant)
      .mockResolvedValueOnce({
        grant_token: 'grant-1',
        expires_at: '2026-07-20T00:00:00Z'
      })
      .mockResolvedValueOnce({
        grant_token: 'grant-2',
        expires_at: '2026-07-20T00:00:00Z'
      });
    const confirmRuntimeWrite = vi.fn().mockResolvedValue(true);
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      baseUrl: 'http://api.test',
      client,
      confirmRuntimeWrite,
      resolveBlockId: () => 'block-1'
    });
    const writeEffect = effect({ method: 'PATCH' });

    await expect(handlers.interface(writeEffect)).resolves.toEqual({ saved: 1 });
    await expect(handlers.interface(writeEffect)).resolves.toEqual({ saved: 2 });

    expect(confirmRuntimeWrite).toHaveBeenCalledTimes(2);
    expect(client.issueFrontstageCallableWriteGrant).toHaveBeenCalledTimes(2);
    expect(client.dispatchFrontstageCallable).toHaveBeenLastCalledWith(
      'workspace-1',
      'page-1',
      'tab-1',
      expect.objectContaining({ write_grant: 'grant-2' }),
      'csrf-1',
      'http://api.test'
    );
  });

  test('B-P1 never confirms or grants a saved runtime GET', async () => {
    const client = createClient();
    vi.mocked(client.dispatchFrontstageCallable).mockRejectedValueOnce(
      writeGrantRequired()
    );
    const confirmRuntimeWrite = vi.fn().mockResolvedValue(true);
    const handlers = createFrontstageJsBlockCapabilityHandlers({
      workspaceId: 'workspace-1',
      pageId: 'page-1',
      tabId: 'tab-1',
      csrfToken: 'csrf-1',
      client,
      confirmRuntimeWrite,
      resolveBlockId: () => 'block-1'
    });

    await expect(handlers.interface(effect())).rejects.toMatchObject({
      code: 'write_grant'
    });
    expect(confirmRuntimeWrite).not.toHaveBeenCalled();
    expect(client.issueFrontstageCallableWriteGrant).not.toHaveBeenCalled();
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

  test('AC-024 confirms once and issues a fresh one-time grant for every write call', async () => {
    const client = createClient();
    vi.mocked(client.dispatchFrontstageCallable)
      .mockRejectedValueOnce(writeGrantRequired())
      .mockResolvedValueOnce({ saved: 1 })
      .mockRejectedValueOnce(writeGrantRequired())
      .mockResolvedValueOnce({ saved: 2 });
    vi.mocked(client.issueFrontstageCallableWriteGrant)
      .mockResolvedValueOnce({
        grant_token: 'grant-1',
        expires_at: '2026-07-20T00:00:00Z'
      })
      .mockResolvedValueOnce({
        grant_token: 'grant-2',
        expires_at: '2026-07-20T00:00:00Z'
      });
    const confirmWrite = vi.fn().mockResolvedValue(true);
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
      runId: 'run-1',
      draftHash: 'draft-1',
      confirmWrite
    });
    const writeEffect = effect({
      method: 'PUT',
      path: '/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}/document',
      request: { body: { payload: {} } }
    });

    await expect(handlers.interface(writeEffect)).resolves.toEqual({
      saved: 1
    });
    await expect(handlers.interface(writeEffect)).resolves.toEqual({
      saved: 2
    });

    expect(confirmWrite).toHaveBeenCalledTimes(1);
    expect(client.issueFrontstageCallableWriteGrant).toHaveBeenCalledTimes(2);
    expect(client.issueFrontstageCallableWriteGrant).toHaveBeenLastCalledWith(
      'workspace-1',
      'page-1',
      'tab-1',
      {
        block_id: 'block-1',
        method: 'PUT',
        path: '/api/console/frontstage/{workspace_id}/pages/{page_id}/tabs/{tab_id}/document',
        run_id: 'run-1',
        draft_hash: 'draft-1'
      },
      'csrf-1',
      'http://api.test'
    );
    expect(client.dispatchFrontstageCallable).toHaveBeenLastCalledWith(
      'workspace-1',
      'page-1',
      'tab-1',
      expect.objectContaining({ write_grant: 'grant-2' }),
      'csrf-1',
      'http://api.test'
    );
  });

  test('does not issue a write grant when the draft confirmation is cancelled', async () => {
    const client = createClient();
    vi.mocked(client.dispatchFrontstageCallable).mockRejectedValue(
      writeGrantRequired()
    );
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
      runId: 'run-1',
      draftHash: 'draft-1',
      confirmWrite: vi.fn().mockResolvedValue(false)
    });

    await expect(
      handlers.interface(effect({ method: 'POST' }))
    ).rejects.toThrow(
      'Write interface call was cancelled.'
    );
    expect(client.dispatchFrontstageCallable).toHaveBeenCalledTimes(1);
    expect(client.issueFrontstageCallableWriteGrant).not.toHaveBeenCalled();
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
      runId: 'run-1',
      draftHash: 'draft-1',
      confirmWrite: vi.fn().mockResolvedValue(true)
    });
    handlers.revokeDraftRun('run-1');

    await expect(
      handlers.interface(effect({ method: 'POST' }))
    ).rejects.toThrow('revoked');
    expect(client.dispatchFrontstageCallable).not.toHaveBeenCalled();
    expect(client.issueFrontstageCallableWriteGrant).not.toHaveBeenCalled();
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

function writeGrantRequired(): ApiClientError {
  return new ApiClientError({
    status: 400,
    code: 'write_grant',
    message: 'write grant required'
  });
}
