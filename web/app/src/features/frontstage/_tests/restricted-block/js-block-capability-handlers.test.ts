import { describe, expect, test, vi } from 'vitest';

import { ApiClientError } from '@1flowbase/api-client';
import type { JsBlockHostInterfaceEffect } from '@1flowbase/page-runtime';
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
  overrides: Partial<JsBlockHostInterfaceEffect> = {}
): JsBlockHostInterfaceEffect {
  return {
    type: 'interface',
    requestId: 'run-1',
    effectId: 'effect-1',
    interfaceId: 'list_application_conversations_records',
    schemaDigest: 'digest-1',
    request: { query: { page: 1 } },
    ...overrides
  };
}

describe('createFrontstageJsBlockCapabilityHandlers', () => {
  test('AC-020 dispatches the source descriptor without resolving a binding', async () => {
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
        interface_id: 'list_application_conversations_records',
        schema_digest: 'digest-1',
        run_id: 'run-1',
        draft_hash: 'runtime',
        request: { query: { page: 1 } }
      },
      'csrf-1',
      'http://api.test'
    );
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
      interfaceId: 'save_frontstage_tab_document',
      schemaDigest: 'digest-write',
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
        interface_id: 'save_frontstage_tab_document',
        schema_digest: 'digest-write',
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

    await expect(handlers.interface(effect())).rejects.toThrow(
      'Write interface call was cancelled.'
    );
    expect(client.issueFrontstageCallableWriteGrant).not.toHaveBeenCalled();
  });

  test('opens, pulls, and cancels an SSE descriptor within the owning run', async () => {
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
    const descriptor = {
      interfaceId: 'stream_application_run_events',
      schemaDigest: 'digest-stream'
    };
    const opened = (await handlers.interface(
      effect({ ...descriptor, operation: 'stream_open', request: undefined })
    )) as { stream_id: string };
    await expect(
      handlers.interface(
        effect({
          ...descriptor,
          operation: 'stream_next',
          streamId: opened.stream_id,
          request: undefined
        })
      )
    ).resolves.toEqual({ done: false, value: { progress: 1 } });
    await expect(
      handlers.interface(
        effect({
          ...descriptor,
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
