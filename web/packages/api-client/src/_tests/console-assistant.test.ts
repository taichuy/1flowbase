import { describe, expect, test, vi } from 'vitest';

import {
  getConsoleAssistantSettings,
  startConsoleAssistantRun,
  startConsoleAssistantRunStream,
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
      { query: 'hello', history: [] },
      'csrf-token',
      { onEvent }
    );

    expect(fetchMock).toHaveBeenCalledWith(
      '/api/console/assistant/runs/stream',
      expect.objectContaining({
        method: 'POST',
        credentials: 'include',
        body: JSON.stringify({ query: 'hello', history: [] }),
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
      { query: 'hello', history: [] },
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
});
