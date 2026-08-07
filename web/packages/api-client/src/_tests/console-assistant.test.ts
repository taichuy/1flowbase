import { describe, expect, test, vi } from 'vitest';

import {
  createConsoleAssistantConversation,
  getConsoleAssistantConversationMessages,
  getConsoleAssistantSettings,
  getConsoleAssistantLegacySnapshotMessages,
  listConsoleAssistantConversations,
  startConsoleAssistantRun,
  startConsoleAssistantRunStream,
  startConsoleAssistantRunWebSocket,
  updateConsoleAssistantSettings
} from '../console-assistant';
import * as transport from '../transport';

describe('console assistant client', () => {
  vi.spyOn(transport, 'apiFetch').mockImplementation(
    async (input) => input as never
  );

  test('AC-002 reads and writes the current assistant preference through the session API', async () => {
    await expect(getConsoleAssistantSettings()).resolves.toMatchObject({
      path: '/api/console/assistant/settings'
    });
    await expect(
      updateConsoleAssistantSettings(
        {
          application_id: 'application-1',
          mcp_instance_ids: ['catalog']
        },
        'csrf-token'
      )
    ).resolves.toMatchObject({
      path: '/api/console/assistant/settings',
      method: 'PATCH',
      csrfToken: 'csrf-token'
    });
  });

  test('AC-003 starts the assistant through its session-only route', async () => {
    await expect(
      startConsoleAssistantRun(
        {
          application_id: 'application-1',
          query: 'hello',
          history: []
        },
        'csrf-token'
      )
    ).resolves.toMatchObject({
      path: '/api/console/assistant/runs',
      method: 'POST',
      csrfToken: 'csrf-token'
    });
  });

  test('issue 1608 uses the dedicated assistant conversation contract', async () => {
    await expect(
      createConsoleAssistantConversation(
        {
          application_id: 'application-1',
          seed_legacy_flow_run_id: 'legacy-run-1'
        },
        'csrf-token'
      )
    ).resolves.toMatchObject({
      path: '/api/console/assistant/conversations',
      method: 'POST',
      csrfToken: 'csrf-token',
      body: {
        application_id: 'application-1',
        seed_legacy_flow_run_id: 'legacy-run-1'
      }
    });
    await expect(
      listConsoleAssistantConversations('application-1', {
        page: 2,
        pageSize: 5
      })
    ).resolves.toMatchObject({
      path: '/api/console/assistant/conversations?application_id=application-1&page=2&page_size=5'
    });
    await expect(
      getConsoleAssistantConversationMessages('application-1', 'conversation-1')
    ).resolves.toMatchObject({
      path: '/api/console/assistant/conversations/conversation-1/messages?application_id=application-1'
    });
    await expect(
      getConsoleAssistantLegacySnapshotMessages('application-1', 'run-1')
    ).resolves.toMatchObject({
      path: '/api/console/assistant/legacy-runs/run-1/messages?application_id=application-1'
    });
  });

  test('AC-003 starts Preview-compatible assistant streaming through the session route', async () => {
    const onEvent = vi.fn();
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        'event: flow_accepted\ndata: {"event_id":"run-1:1","run_id":"run-1","event_type":"flow_accepted","sequence":1,"created_at":"2026-08-05T00:00:00Z","payload":{"type":"flow_accepted","run_id":"run-1","status":"queued"}}\n\n',
        {
          status: 200,
          headers: { 'content-type': 'text/event-stream' }
        }
      )
    );

    await startConsoleAssistantRunStream(
      { application_id: 'application-1', query: 'hello', history: [] },
      'csrf-token',
      { onEvent }
    );

    expect(fetchMock).toHaveBeenCalledWith(
      '/api/console/assistant/runs/stream',
      expect.objectContaining({
        method: 'POST',
        credentials: 'include',
        body: JSON.stringify({
          application_id: 'application-1',
          query: 'hello',
          history: []
        }),
        headers: expect.objectContaining({
          accept: 'text/event-stream',
          'x-csrf-token': 'csrf-token'
        })
      })
    );
    expect(onEvent).toHaveBeenCalledWith(
      expect.objectContaining({ type: 'flow_accepted', run_id: 'run-1' })
    );
  });

  test('AC-004 keeps a published Flow incomplete terminal visible to the Preview', async () => {
    const onEvent = vi.fn();
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        'event: flow_incomplete\ndata: {"event_id":"run-1:2","run_id":"run-1","event_type":"flow_incomplete","sequence":2,"created_at":"2026-08-05T00:00:00Z","payload":{"type":"flow_incomplete","run_id":"run-1","status":"incomplete","reason":"output_limit","output":{"answer":"Partial answer"}}}\n\n',
        {
          status: 200,
          headers: { 'content-type': 'text/event-stream' }
        }
      )
    );

    await startConsoleAssistantRunStream(
      { application_id: 'application-1', query: 'hello', history: [] },
      'csrf-token',
      { onEvent }
    );

    expect(onEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'flow_incomplete',
        run_id: 'run-1',
        status: 'incomplete',
        output: { answer: 'Partial answer' }
      })
    );
  });

  test('issue 1601 streams multiple Assistant WebSocket deltas before terminal', async () => {
    const sent: string[] = [];
    const onEvent = vi.fn();
    vi.mocked(transport.apiFetch).mockResolvedValueOnce({
      ticket: 'ticket-1',
      protocol: '1flowbase.assistant.v1',
      expires_in_seconds: 60
    } as never);

    class FakeWebSocket {
      static readonly OPEN = 1;
      readonly readyState = FakeWebSocket.OPEN;
      onopen: (() => void) | null = null;
      onmessage: ((event: MessageEvent) => void) | null = null;
      onerror: (() => void) | null = null;
      onclose: (() => void) | null = null;

      constructor(
        readonly url: URL,
        readonly protocols: string[]
      ) {
        queueMicrotask(() => this.onopen?.());
      }

      send(value: string) {
        sent.push(value);
        const command = JSON.parse(value) as { type: string };
        if (command.type !== 'run.create') {
          return;
        }
        for (const event of [
          {
            type: 'flow_accepted',
            event_type: 'flow_accepted',
            event_id: 'run-1:1',
            run_id: 'run-1',
            sequence: 1,
            created_at: '2026-08-06T00:00:00Z',
            payload: { status: 'queued' }
          },
          {
            type: 'context_snapshot',
            event_type: 'context_snapshot',
            event_id: 'run-1:2',
            run_id: 'run-1',
            sequence: 2,
            created_at: '2026-08-06T00:00:01Z',
            payload: {
              node_id: 'llm',
              node_run_id: 'node-run-1',
              input_tokens: 321,
              effective_context_window: 128000,
              remaining_tokens: 127679,
              measurement: {
                method: 'generic_estimate',
                accuracy: 'estimated',
                coverage: 'complete',
                unknown_block_count: 0
              }
            }
          },
          {
            type: 'text_delta',
            event_type: 'text_delta',
            event_id: 'run-1:3',
            run_id: 'run-1',
            sequence: 3,
            created_at: '2026-08-06T00:00:02Z',
            text: 'Hel',
            payload: { node_id: 'answer', text: 'Hel' }
          },
          {
            type: 'text_delta',
            event_type: 'text_delta',
            event_id: 'run-1:4',
            run_id: 'run-1',
            sequence: 4,
            created_at: '2026-08-06T00:00:03Z',
            text: 'lo',
            payload: { node_id: 'answer', text: 'lo' }
          },
          {
            type: 'flow_finished',
            event_type: 'flow_finished',
            event_id: 'run-1:5',
            run_id: 'run-1',
            sequence: 5,
            created_at: '2026-08-06T00:00:04Z',
            payload: { status: 'succeeded', output: { answer: 'Hello' } }
          }
        ]) {
          this.onmessage?.({ data: JSON.stringify(event) } as MessageEvent);
        }
      }

      close() {
        this.onclose?.();
      }
    }

    vi.stubGlobal('WebSocket', FakeWebSocket);
    await startConsoleAssistantRunWebSocket(
      { application_id: 'application-1', query: 'hello', history: [] },
      'csrf-token',
      { onEvent },
      { baseUrl: 'http://127.0.0.1:3100' }
    );

    expect(sent[0]).toContain('"type":"run.create"');
    expect(transport.apiFetch).toHaveBeenCalledWith(
      expect.objectContaining({
        path: '/api/console/assistant/runs/websocket-ticket',
        method: 'POST',
        body: { application_id: 'application-1' },
        csrfToken: 'csrf-token'
      })
    );
    expect(
      onEvent.mock.calls
        .map(([event]) => event)
        .find((event) => event.type === 'context_snapshot')
    ).toMatchObject({
      input_tokens: 321,
      effective_context_window: 128000,
      measurement: {
        method: 'generic_estimate',
        accuracy: 'estimated'
      }
    });
    expect(
      onEvent.mock.calls
        .map(([event]) => event)
        .filter((event) => event.type === 'text_delta')
        .map((event) => event.text)
    ).toEqual(['Hel', 'lo']);
    vi.unstubAllGlobals();
  });

  test('issue 1601 reconnects with run.attach and the last event id', async () => {
    const sent: Array<Record<string, unknown>> = [];
    const onEvent = vi.fn();
    vi.mocked(transport.apiFetch)
      .mockResolvedValueOnce({
        ticket: 'ticket-1',
        protocol: '1flowbase.assistant.v1',
        expires_in_seconds: 60
      } as never)
      .mockResolvedValueOnce({
        ticket: 'ticket-2',
        protocol: '1flowbase.assistant.v1',
        expires_in_seconds: 60
      } as never);
    let connection = 0;

    class ReconnectingWebSocket {
      static readonly OPEN = 1;
      readonly readyState = ReconnectingWebSocket.OPEN;
      onopen: (() => void) | null = null;
      onmessage: ((event: MessageEvent) => void) | null = null;
      onerror: (() => void) | null = null;
      onclose: (() => void) | null = null;
      readonly connection = ++connection;

      constructor(
        readonly url: URL,
        readonly protocols: string[]
      ) {
        queueMicrotask(() => this.onopen?.());
      }

      send(value: string) {
        const command = JSON.parse(value) as Record<string, unknown>;
        sent.push(command);
        if (this.connection === 1 && command.type === 'run.create') {
          this.onmessage?.({
            data: JSON.stringify({
              type: 'flow_accepted',
              event_type: 'flow_accepted',
              event_id: 'run-1:1',
              run_id: 'run-1',
              sequence: 1,
              payload: { status: 'queued' }
            })
          } as MessageEvent);
          this.onmessage?.({
            data: JSON.stringify({
              type: 'text_delta',
              event_type: 'text_delta',
              event_id: 'run-1:2',
              run_id: 'run-1',
              sequence: 2,
              text: 'Hel',
              payload: { node_id: 'answer', text: 'Hel' }
            })
          } as MessageEvent);
          queueMicrotask(() => this.onclose?.());
          return;
        }
        if (this.connection === 2 && command.type === 'run.attach') {
          this.onmessage?.({
            data: JSON.stringify({
              type: 'text_delta',
              event_type: 'text_delta',
              event_id: 'run-1:3',
              run_id: 'run-1',
              sequence: 3,
              text: 'lo',
              payload: { node_id: 'answer', text: 'lo' }
            })
          } as MessageEvent);
          this.onmessage?.({
            data: JSON.stringify({
              type: 'flow_finished',
              event_type: 'flow_finished',
              event_id: 'run-1:4',
              run_id: 'run-1',
              sequence: 4,
              payload: { status: 'succeeded', output: { answer: 'Hello' } }
            })
          } as MessageEvent);
        }
      }

      close() {
        this.onclose?.();
      }
    }

    vi.stubGlobal('WebSocket', ReconnectingWebSocket);
    await startConsoleAssistantRunWebSocket(
      { application_id: 'application-1', query: 'hello', history: [] },
      'csrf-token',
      { onEvent },
      { baseUrl: 'http://127.0.0.1:3100' }
    );

    expect(sent).toContainEqual(
      expect.objectContaining({
        type: 'run.attach',
        run_id: 'run-1',
        after_event_id: 'run-1:2'
      })
    );
    expect(
      onEvent.mock.calls
        .map(([event]) => event)
        .filter((event) => event.type === 'text_delta')
        .map((event) => event.text)
    ).toEqual(['Hel', 'lo']);
    vi.unstubAllGlobals();
  });

  test('issue 1601 rejects a WebSocket handshake that never opens', async () => {
    vi.useFakeTimers();
    vi.mocked(transport.apiFetch).mockResolvedValueOnce({
      ticket: 'ticket-stalled',
      protocol: '1flowbase.assistant.v1',
      expires_in_seconds: 60
    } as never);
    const close = vi.fn();

    class StalledWebSocket {
      static readonly OPEN = 1;
      readonly readyState = 0;
      onopen: (() => void) | null = null;
      onmessage: ((event: MessageEvent) => void) | null = null;
      onerror: (() => void) | null = null;
      onclose: (() => void) | null = null;

      close() {
        close();
      }

      send() {}
    }

    vi.stubGlobal('WebSocket', StalledWebSocket);
    const run = startConsoleAssistantRunWebSocket(
      { application_id: 'application-1', query: 'hello', history: [] },
      'csrf-token',
      {},
      {
        baseUrl: 'http://127.0.0.1:3100',
        handshakeTimeoutMs: 25
      }
    );

    const rejection = expect(run).rejects.toThrow(
      'Assistant WebSocket handshake timed out'
    );
    await vi.advanceTimersByTimeAsync(25);
    await rejection;
    expect(close).toHaveBeenCalledTimes(1);
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });
});
