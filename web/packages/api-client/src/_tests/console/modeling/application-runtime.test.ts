import { describe, test, expect, vi } from 'vitest';
import {
  completeConsoleRunArchiveUploadSession,
  createConsoleApplicationRunsArchive,
  createConsoleRunArchiveUploadSession,
  exportConsoleApplicationRunTraceDump,
  exportConsoleApplicationRunsTraceDumpZip,
  getConsoleApplicationRunArchive,
  getConsoleRunArchiveImportJob,
  getConsoleRuntimeDebugArtifact,
  getConsoleRuntimeDebugStream,
  normalizeConsoleRuntimeEvent,
  sseResponse,
  startConsoleFlowDebugRunStream,
  subscribeConsoleFlowDebugRunStream,
  uploadConsoleRunArchiveChunk
} from './application-runtime.fixtures';

describe('console application runtime stream client', () => {
  test('AC-010 accepts canonical tool calls and rejects compatibility aliases', () => {
    expect(
      normalizeConsoleRuntimeEvent({
        event_type: 'assistant_tool_call_started',
        event_id: 'run-1:2',
        run_id: 'run-1',
        sequence: 2,
        created_at: '2026-08-16T00:00:00Z',
        payload: {
          node_id: 'node-llm',
          tool_call: {
            id: 'call-1',
            name: '1flowbase_mcp_get',
            arguments: { group_id: 'group-123' }
          },
          call_usage: {
            input_tokens: 1200,
            output_tokens: 260,
            total_tokens: 1460
          }
        }
      })
    ).toMatchObject({
      type: 'assistant_tool_call_started',
      tool_call: {
        id: 'call-1',
        name: '1flowbase_mcp_get',
        arguments: { group_id: 'group-123' }
      },
      call_usage: {
        input_tokens: 1200,
        output_tokens: 260,
        total_tokens: 1460
      }
    });
    expect(
      normalizeConsoleRuntimeEvent({
        event_type: 'assistant_tool_call_started',
        event_id: 'run-1:3',
        run_id: 'run-1',
        sequence: 3,
        created_at: '2026-08-16T00:00:01Z',
        payload: {
          node_id: 'node-llm',
          tool_call: {
            tool_call_id: 'legacy-call',
            function: { name: 'legacy-tool' }
          }
        }
      })
    ).toBeNull();
  });

  test('AC-001 preserves Answer presentation metadata from runtime envelopes', async () => {
    const onEvent = vi.fn();
    const onCompleted = vi.fn();
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      sseResponse(`id: run-1:2
event: text_delta
data: {"event_id":"run-1:2","run_id":"run-1","node_run_id":null,"event_type":"text_delta","sequence":2,"created_at":"2026-05-08T00:00:00Z","delta_index":2,"content_type":"text","text":"退款","payload":{"type":"text_delta","node_id":"node-answer","text":"退款","presentation":{"kind":"answer","answer_node_id":"node-answer","segment_index":0,"source_node_id":"node-llm","source_node_run_id":"node-run-1","source_output_key":"text"}}}

`)
    );

    await startConsoleFlowDebugRunStream(
      'app-1',
      { input_payload: { 'node-start': { query: '退款' } } },
      'csrf-123',
      { onEvent, onCompleted },
      {
        baseUrl: 'http://127.0.0.1:7800',
        cursor: { from_sequence: 1, last_event_id: 'run-1:1' }
      }
    );

    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/orchestration/debug-runs/stream?from_sequence=1&last_event_id=run-1%3A1',
      expect.objectContaining({
        body: JSON.stringify({
          input_payload: { 'node-start': { query: '退款' } }
        })
      })
    );
    expect(onEvent).toHaveBeenCalledWith({
      type: 'text_delta',
      run_id: 'run-1',
      node_run_id: null,
      node_id: 'node-answer',
      text: '退款',
      presentation: {
        kind: 'answer',
        answer_node_id: 'node-answer',
        segment_index: 0,
        source_node_id: 'node-llm',
        source_node_run_id: 'node-run-1',
        source_output_key: 'text'
      },
      event_id: 'run-1:2',
      sequence: 2,
      created_at: '2026-05-08T00:00:00Z',
      delta_index: 2,
      content_type: 'text'
    });
    expect(onCompleted).toHaveBeenCalledTimes(1);
  });

  test('subscribes to an existing run stream with cursor query', async () => {
    const onEvent = vi.fn();
    const onCompleted = vi.fn();
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(
        sseResponse(
          'id: run-1:42\nevent: replay_expired\ndata: {"type":"replay_expired","run_id":"run-1","from_sequence":42,"reason":"cursor_expired"}\n\n'
        )
      );

    await subscribeConsoleFlowDebugRunStream(
      'app-1',
      'run-1',
      'csrf-123',
      { onEvent, onCompleted },
      {
        baseUrl: 'http://127.0.0.1:7800',
        cursor: { from_sequence: 42 }
      }
    );

    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/orchestration/runs/run-1/debug-stream?from_sequence=42',
      expect.objectContaining({
        method: 'GET',
        headers: expect.objectContaining({
          accept: 'text/event-stream',
          'x-csrf-token': 'csrf-123'
        })
      })
    );
    expect(onEvent).toHaveBeenCalledWith({
      type: 'replay_expired',
      run_id: 'run-1',
      from_sequence: 42,
      reason: 'cursor_expired',
      event_id: 'run-1:42'
    });
    expect(onCompleted).toHaveBeenCalledTimes(1);
  });

  test('loads runtime debug artifact content by application scope', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response('{"hello":"world"}', {
        status: 200,
        headers: { 'content-type': 'application/json' }
      })
    );

    await expect(
      getConsoleRuntimeDebugArtifact(
        'app-1',
        'artifact-1',
        'http://127.0.0.1:7800'
      )
    ).resolves.toEqual({ hello: 'world' });
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/orchestration/debug-artifacts/artifact-1',
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
  });

  test('loads runtime debug stream with sequence cursor and limit', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          data: {
            parts: [],
            page_size: 25,
            next_sequence: 42,
            has_more: true
          }
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      )
    );

    await expect(
      getConsoleRuntimeDebugStream('app-1', 'run-1', 'http://127.0.0.1:7800', {
        from_sequence: 12,
        limit: 25
      })
    ).resolves.toEqual({
      parts: [],
      page_size: 25,
      next_sequence: 42,
      has_more: true
    });
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/run-1/debug-stream?from_sequence=12&limit=25',
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
  });

  test('exports a single run trace dump as a blob response with filename metadata', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        new Blob(['{"run_id":"run-1"}'], { type: 'application/json' }),
        {
          status: 200,
          headers: {
            'content-type': 'application/json',
            'content-disposition':
              'attachment; filename="application-run-run-1-trace.json"'
          }
        }
      )
    );

    const response = await exportConsoleApplicationRunTraceDump(
      'app-1',
      'run-1',
      'http://127.0.0.1:7800'
    );

    await expect(response.blob.text()).resolves.toBe('{"run_id":"run-1"}');
    expect(response.filename).toBe('application-run-run-1-trace.json');
    expect(response.contentType).toBe('application/json');
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/run-1/export',
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
  });

  test('exports selected run trace dumps as a csrf-protected zip blob response', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(new Blob(['zip-bytes'], { type: 'application/zip' }), {
        status: 200,
        headers: {
          'content-type': 'application/zip',
          'content-disposition':
            "attachment; filename*=UTF-8''selected-traces.zip"
        }
      })
    );

    const response = await exportConsoleApplicationRunsTraceDumpZip(
      'app-1',
      ['run-1', 'run-2'],
      'csrf-123',
      'http://127.0.0.1:7800'
    );

    await expect(response.blob.text()).resolves.toBe('zip-bytes');
    expect(response.filename).toBe('selected-traces.zip');
    expect(response.contentType).toBe('application/zip');
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/export',
      expect.objectContaining({
        method: 'POST',
        credentials: 'include',
        body: JSON.stringify({
          run_ids: ['run-1', 'run-2']
        }),
        headers: expect.objectContaining({
          'content-type': 'application/json',
          'x-csrf-token': 'csrf-123'
        })
      })
    );
  });

  test('loads run archive JSON with explicit archive version', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          archive_version: 1,
          exported_at: '2026-06-24T00:00:00Z',
          manifest: {
            archive_version: 1,
            archive_semantics: 'application_run_archive_v1',
            exported_at: '2026-06-24T00:00:00Z',
            source_workspace_id: 'workspace-1',
            source_application_id: 'app-1',
            run_count: 1,
            selected_run_ids: ['run-1'],
            entries: [],
            content_sha256: 'sha256:archive',
            checksum: 'sha256:archive'
          },
          source: {
            source_kind: 'application_run',
            application_id: 'app-1',
            application_type: 'agent_flow',
            application_name: 'Agent',
            workspace_id: 'workspace-1',
            exported_by_user_id: 'user-1',
            exported_at: '2026-06-24T00:00:00Z',
            archive_builder: 'application_run_archive_v1'
          },
          entries: [],
          content_digest: 'sha256:archive'
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      )
    );

    await expect(
      getConsoleApplicationRunArchive(
        'app-1',
        'run-1',
        'http://127.0.0.1:7800',
        { archive_version: 1 }
      )
    ).resolves.toEqual(
      expect.objectContaining({
        archive_version: 1,
        content_digest: 'sha256:archive'
      })
    );
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/run-1/archive?archive_version=1',
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
  });

  test('creates selected runs archive as csrf-protected JSON', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          archive_version: 1,
          exported_at: '2026-06-24T00:00:00Z',
          manifest: {
            archive_version: 1,
            archive_semantics: 'application_run_archive_v1',
            exported_at: '2026-06-24T00:00:00Z',
            source_workspace_id: 'workspace-1',
            source_application_id: 'app-1',
            run_count: 2,
            selected_run_ids: ['run-1', 'run-2'],
            entries: [],
            content_sha256: 'sha256:archive',
            checksum: 'sha256:archive'
          },
          source: {
            source_kind: 'application_run',
            application_id: 'app-1',
            application_type: 'agent_flow',
            application_name: 'Agent',
            workspace_id: 'workspace-1',
            exported_by_user_id: 'user-1',
            exported_at: '2026-06-24T00:00:00Z',
            archive_builder: 'application_run_archive_v1'
          },
          entries: [],
          content_digest: 'sha256:archive'
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      )
    );

    await expect(
      createConsoleApplicationRunsArchive(
        'app-1',
        ['run-1', 'run-2'],
        'csrf-123',
        'http://127.0.0.1:7800',
        { archive_version: 1 }
      )
    ).resolves.toEqual(
      expect.objectContaining({
        archive_version: 1,
        content_digest: 'sha256:archive'
      })
    );
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/archive',
      expect.objectContaining({
        method: 'POST',
        credentials: 'include',
        body: JSON.stringify({
          archive_version: 1,
          run_ids: ['run-1', 'run-2']
        }),
        headers: expect.objectContaining({
          'content-type': 'application/json',
          'x-csrf-token': 'csrf-123'
        })
      })
    );
  });

  test('creates archive upload sessions with csrf protected metadata', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          data: {
            session_id: 'session-1',
            application_id: 'app-1',
            status: 'uploading',
            filename: 'archive.json',
            total_size_bytes: 42,
            received_bytes: 0,
            expected_sha256: 'sha256:archive',
            created_at: '2026-06-24T00:00:00Z',
            updated_at: '2026-06-24T00:00:00Z'
          },
          meta: null
        }),
        {
          status: 201,
          headers: { 'content-type': 'application/json' }
        }
      )
    );

    await expect(
      createConsoleRunArchiveUploadSession(
        'app-1',
        {
          filename: 'archive.json',
          total_size_bytes: 42,
          expected_sha256: 'sha256:archive',
          chunk_size_bytes: 1024
        },
        'csrf-123',
        'http://127.0.0.1:7800'
      )
    ).resolves.toMatchObject({
      session_id: 'session-1',
      status: 'uploading'
    });
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/archive/import-sessions',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          filename: 'archive.json',
          total_size_bytes: 42,
          expected_sha256: 'sha256:archive',
          chunk_size_bytes: 1024
        }),
        headers: expect.objectContaining({
          'content-type': 'application/json',
          'x-csrf-token': 'csrf-123'
        })
      })
    );
  });

  test('uploads archive chunks as raw octets with chunk checksum', async () => {
    const chunk = new Blob(['part-1']);
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          data: {
            session_id: 'session-1',
            chunk_index: 0,
            chunk_size_bytes: 6,
            chunk_sha256: 'sha256:chunk',
            received_bytes: 6,
            status: 'uploading'
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      )
    );

    await expect(
      uploadConsoleRunArchiveChunk(
        'app-1',
        'session-1',
        0,
        chunk,
        'sha256:chunk',
        'csrf-123',
        'http://127.0.0.1:7800'
      )
    ).resolves.toMatchObject({
      chunk_index: 0,
      received_bytes: 6
    });
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/archive/import-sessions/session-1/chunks/0',
      expect.objectContaining({
        method: 'PUT',
        body: chunk,
        headers: expect.objectContaining({
          'content-type': 'application/octet-stream',
          'x-chunk-sha256': 'sha256:chunk',
          'x-csrf-token': 'csrf-123'
        })
      })
    );
  });

  test('completes archive upload sessions and polls import jobs', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            data: {
              job_id: 'job-1',
              application_id: 'app-1',
              upload_session_id: 'session-1',
              status: 'queued',
              archive_version: 1,
              archive_sha256: 'sha256:archive',
              run_count: 1,
              imported_run_count: 0,
              source_to_target_run_ids: [],
              error_payload: null,
              result_payload: {},
              created_at: '2026-06-24T00:00:00Z',
              updated_at: '2026-06-24T00:00:00Z',
              started_at: null,
              finished_at: null
            },
            meta: null
          }),
          {
            status: 200,
            headers: { 'content-type': 'application/json' }
          }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            data: {
              job_id: 'job-1',
              application_id: 'app-1',
              upload_session_id: 'session-1',
              status: 'succeeded',
              archive_version: 1,
              archive_sha256: 'sha256:archive',
              run_count: 1,
              imported_run_count: 1,
              source_to_target_run_ids: [
                { source_run_id: 'run-source', target_run_id: 'run-target' }
              ],
              error_payload: null,
              result_payload: {},
              created_at: '2026-06-24T00:00:00Z',
              updated_at: '2026-06-24T00:00:01Z',
              started_at: '2026-06-24T00:00:00Z',
              finished_at: '2026-06-24T00:00:01Z'
            },
            meta: null
          }),
          {
            status: 200,
            headers: { 'content-type': 'application/json' }
          }
        )
      );

    await expect(
      completeConsoleRunArchiveUploadSession(
        'app-1',
        'session-1',
        'csrf-123',
        'http://127.0.0.1:7800'
      )
    ).resolves.toMatchObject({
      job_id: 'job-1',
      status: 'queued'
    });
    await expect(
      getConsoleRunArchiveImportJob('app-1', 'job-1', 'http://127.0.0.1:7800')
    ).resolves.toMatchObject({
      job_id: 'job-1',
      status: 'succeeded',
      imported_run_count: 1
    });
    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/archive/import-sessions/session-1/complete',
      expect.objectContaining({
        method: 'POST',
        headers: expect.objectContaining({
          'x-csrf-token': 'csrf-123'
        })
      })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/archive/import-jobs/job-1',
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
  });
});
