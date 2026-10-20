import { describe, test, expect, vi } from 'vitest';
import './application-runtime.fixtures';
import {
  getConsoleApplicationConversationMessages,
  getConsoleApplicationRunConversationMessages,
  getConsoleApplicationRunDebugSnapshot,
  getConsoleApplicationRunMonitoringReport,
  getConsoleApplicationRunNodeLastRun,
  getConsoleApplicationRunResumeTimeline,
  getConsoleApplicationRunResumeTimelineSummary,
  getConsoleApplicationRunTraceNodeChildren,
  getConsoleApplicationRunTraceNodeContent,
  getConsoleApplicationRunTraceNodeDetail,
  getConsoleApplicationRunTraceToolCallbackContent,
  getConsoleApplicationRunTraceTree,
  getConsoleApplicationRuns,
  getConsoleApplicationRuntimeActivity,
  getConsoleDebugVariableSnapshot,
  resolveConsoleRuntimeDebugArtifacts
} from '../../../console/application-runtime';

describe('console application runtime stream client', () => {
  test('resolves runtime debug artifacts in one request', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          data: {
            artifacts: [
              {
                artifact_ref: 'artifact-1',
                content_type: 'application/json',
                value: { hello: 'world' }
              }
            ]
          }
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      )
    );

    await expect(
      resolveConsoleRuntimeDebugArtifacts(
        'app-1',
        ['artifact-1', 'artifact-2'],
        'http://127.0.0.1:7800'
      )
    ).resolves.toEqual({
      artifacts: [
        {
          artifact_ref: 'artifact-1',
          content_type: 'application/json',
          value: { hello: 'world' }
        }
      ]
    });
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/orchestration/debug-artifacts/resolve',
      expect.objectContaining({
        method: 'POST',
        credentials: 'include',
        body: JSON.stringify({
          artifact_refs: ['artifact-1', 'artifact-2']
        })
      })
    );
  });

  test('loads debug variable snapshot without query parameters', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response('{"data":{"variable_cache":{}}}', {
        status: 200,
        headers: { 'content-type': 'application/json' }
      })
    );

    await expect(
      getConsoleDebugVariableSnapshot('app-1', 'http://127.0.0.1:7800')
    ).resolves.toEqual({ variable_cache: {} });
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/orchestration/debug-variable-snapshot',
      expect.any(Object)
    );
  });

  test('fetches node run detail inside an explicit flow run', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ data: null }), {
        status: 200,
        headers: { 'content-type': 'application/json' }
      })
    );

    await expect(
      getConsoleApplicationRunNodeLastRun(
        'app-1',
        'run-1',
        'node-llm',
        'http://127.0.0.1:7800'
      )
    ).resolves.toEqual(null);

    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/run-1/nodes/node-llm',
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
  });

  test('loads a debug session snapshot from the orchestration plane', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ data: { flow_run: { id: 'run-1' } } }), {
        status: 200,
        headers: { 'content-type': 'application/json' }
      })
    );

    await expect(
      getConsoleApplicationRunDebugSnapshot(
        'app-1',
        'run-1',
        'http://127.0.0.1:7800'
      )
    ).resolves.toEqual({ flow_run: { id: 'run-1' } });

    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/orchestration/runs/run-1/debug-snapshot',
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
  });

  test('fetches lazy run trace tree resources', async () => {
    const traceNodeId = '11111111-1111-4111-8111-111111111111';
    const projectionStatus = {
      projection_status: 'succeeded',
      projection_version: 1,
      source_watermark: 'run-1:1',
      attempt_count: 1,
      last_attempt_at: '2026-05-08T00:00:00Z',
      last_success_at: '2026-05-08T00:00:01Z',
      last_error_code: null,
      last_error_stage: null,
      last_error_source_kind: null,
      last_error_source_locator: null,
      last_error_ref: null,
      retriable: false
    };
    const traceResponses = [
      { projection_status: projectionStatus, nodes: [] },
      {
        projection_status: projectionStatus,
        items: [],
        page_info: {
          has_more: true,
          next_cursor: 'cursor-page-2',
          page_size: 20
        }
      },
      {
        trace_node_id: traceNodeId,
        node_kind: 'node_run',
        projection_status: projectionStatus,
        content_kind: 'node_run',
        source_refs: [],
        detail_refs: [],
        payload: {}
      },
      {
        trace_node_id: traceNodeId,
        projection_status: projectionStatus,
        tool_call_id: 'call/weather',
        payload: {
          ok: true
        }
      },
      {
        trace_node_id: traceNodeId,
        node_kind: 'node_run',
        projection_status: projectionStatus,
        detail_ref_id: 'node_run',
        detail_kind: 'node_run',
        source_refs: [],
        payload: {
          node_run: {
            id: 'node-run-1'
          }
        }
      },
      { nodes: [] },
      { nodes: [] }
    ];
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(() =>
      Promise.resolve(
        new Response(JSON.stringify({ data: traceResponses.shift() }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
      )
    );

    await expect(
      getConsoleApplicationRunTraceTree(
        'app-1',
        'run-1',
        'http://127.0.0.1:7800'
      )
    ).resolves.toEqual({
      projection_status: projectionStatus,
      nodes: []
    });
    await expect(
      getConsoleApplicationRunTraceNodeChildren(
        'app-1',
        'run-1',
        traceNodeId,
        'http://127.0.0.1:7800',
        {
          cursor: 'cursor-page-1',
          page_size: 20
        }
      )
    ).resolves.toEqual({
      projection_status: projectionStatus,
      items: [],
      page_info: {
        has_more: true,
        next_cursor: 'cursor-page-2',
        page_size: 20
      }
    });
    await expect(
      getConsoleApplicationRunTraceNodeContent(
        'app-1',
        'run-1',
        traceNodeId,
        'http://127.0.0.1:7800',
        {
          artifact_preview: 'auto'
        }
      )
    ).resolves.toEqual({
      trace_node_id: traceNodeId,
      node_kind: 'node_run',
      projection_status: projectionStatus,
      content_kind: 'node_run',
      source_refs: [],
      detail_refs: [],
      payload: {}
    });
    await expect(
      getConsoleApplicationRunTraceToolCallbackContent(
        'app-1',
        'run-1',
        traceNodeId,
        'call/weather',
        'http://127.0.0.1:7800'
      )
    ).resolves.toEqual({
      trace_node_id: traceNodeId,
      tool_call_id: 'call/weather',
      projection_status: projectionStatus,
      payload: {
        ok: true
      }
    });
    await expect(
      getConsoleApplicationRunTraceNodeDetail(
        'app-1',
        'run-1',
        traceNodeId,
        'node_run',
        'http://127.0.0.1:7800',
        {
          artifact_preview: 'auto',
          artifact_preview_field: [
            'node_run.debug_payload.llm_rounds',
            'node_run.output_payload'
          ]
        }
      )
    ).resolves.toEqual({
      trace_node_id: traceNodeId,
      node_kind: 'node_run',
      projection_status: projectionStatus,
      detail_ref_id: 'node_run',
      detail_kind: 'node_run',
      source_refs: [],
      payload: {
        node_run: {
          id: 'node-run-1'
        }
      }
    });
    await expect(
      getConsoleApplicationRunResumeTimeline(
        'app-1',
        'run-1',
        'http://127.0.0.1:7800'
      )
    ).resolves.toEqual({ nodes: [] });
    await expect(
      getConsoleApplicationRunResumeTimelineSummary(
        'app-1',
        'run-1',
        'http://127.0.0.1:7800'
      )
    ).resolves.toEqual({ nodes: [] });

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/run-1/trace-tree',
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      `http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/run-1/trace-tree/nodes?parent_trace_node_id=${traceNodeId}&cursor=cursor-page-1&page_size=20`,
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      `http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/run-1/trace-tree/nodes/${traceNodeId}/content?artifact_preview=auto`,
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      4,
      `http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/run-1/trace-tree/nodes/${traceNodeId}/tool-callbacks/call%2Fweather/content`,
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      5,
      `http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/run-1/trace-tree/nodes/${traceNodeId}/details/node_run?artifact_preview=auto&artifact_preview_field=node_run.debug_payload.llm_rounds&artifact_preview_field=node_run.output_payload`,
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      6,
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/run-1/resume-timeline',
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      7,
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/run-1/resume-timeline-summary',
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
  });

  test('fetches conversation messages around an explicit flow run', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ data: { items: [], page: {} } }), {
        status: 200,
        headers: { 'content-type': 'application/json' }
      })
    );

    await expect(
      getConsoleApplicationRunConversationMessages(
        'app-1',
        'run-1',
        { limit: 5 },
        'http://127.0.0.1:7800'
      )
    ).resolves.toEqual({ items: [], page: {} });

    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs/run-1/conversation/messages?limit=5',
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
  });

  test('fetches external conversation messages around an explicit flow run', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ data: { items: [], page: {} } }), {
        status: 200,
        headers: { 'content-type': 'application/json' }
      })
    );

    await expect(
      getConsoleApplicationConversationMessages(
        'app-1',
        'conversation 1',
        { around_run_id: 'run-2', limit: 5 },
        'http://127.0.0.1:7800'
      )
    ).resolves.toEqual({ items: [], page: {} });

    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/conversations/conversation%201/messages?around_run_id=run-2&limit=5',
      expect.objectContaining({
        method: 'GET',
        credentials: 'include'
      })
    );
  });

  test('fetches application run monitoring report by started_at window', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          data: {
            meta: {
              started_from: '2026-05-01T00:00:00Z',
              started_to: null,
              bucket: 'day',
              slow_run_threshold_ms: 30000
            },
            overview: {
              total_count: 2,
              success_count: 1,
              failed_count: 1,
              cancelled_count: 0,
              success_rate: 0.5,
              failed_rate: 0.5,
              running_count_included: false
            },
            duration: {
              duration_recorded_count: 2,
              avg_duration_ms: 22500,
              p50_duration_ms: 22500,
              p95_duration_ms: 38250,
              slow_run_rate: 0.5
            },
            tokens: {
              total_tokens_sum: 500,
              input_tokens_sum: 380,
              output_tokens_sum: 120,
              input_cache_hit_tokens_sum: 60,
              avg_tokens_per_run: 250,
              token_recorded_count: 2
            },
            tokens_comparison: {
              previous_total_tokens_sum: 300,
              previous_run_count: 1,
              previous_avg_tokens_per_run: 300,
              token_change_rate: 0.6666666667,
              run_count_change_rate: 1,
              avg_tokens_per_run_change_rate: -0.1666666667,
              traffic_effect: 2,
              cost_per_run_effect: 0.8333333333
            },
            tool_callbacks: {
              total_tool_callback_count: 2,
              avg_tool_callback_count: 1,
              runs_with_tool_callback: 1
            },
            nodes: {
              avg_unique_node_count: 1.5,
              max_unique_node_count: 2
            },
            concurrency: {
              peak_concurrency: 2
            },
            tokens_trend: [
              {
                bucket_start: '2026-05-01T00:00:00Z',
                run_count: 2,
                total_tokens: 500,
                input_tokens: 380,
                output_tokens: 120,
                input_cache_hit_tokens: 60
              }
            ],
            protocols: [],
            sources: [],
            authorized_accounts: [],
            api_keys: [],
            external_conversations: [],
            slowest_runs: [],
            high_token_runs: []
          }
        }),
        { status: 200, headers: { 'content-type': 'application/json' } }
      )
    );

    await expect(
      getConsoleApplicationRunMonitoringReport(
        'app-1',
        {
          time_range_days: 7,
          bucket: 'day'
        },
        'http://127.0.0.1:7800'
      )
    ).resolves.toMatchObject({
      overview: {
        total_count: 2,
        running_count_included: false
      },
      duration: {
        slow_run_rate: 0.5
      },
      tokens_comparison: {
        previous_total_tokens_sum: 300
      },
      tokens_trend: [
        {
          input_tokens: 380,
          output_tokens: 120,
          input_cache_hit_tokens: 60
        }
      ],
      tokens: {
        input_tokens_sum: 380,
        output_tokens_sum: 120,
        input_cache_hit_tokens_sum: 60
      }
    });

    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/monitoring/run-metrics?time_range_days=7&bucket=day',
      expect.objectContaining({ method: 'GET' })
    );
  });

  test('fetches application runtime activity without historical range query', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          data: {
            meta: {
              application_id: 'app-1',
              scope: 'current_instance',
              storage: 'memory',
              instance_started_at: '2026-05-30T00:00:00Z',
              snapshot_at: '2026-05-30T00:01:00Z'
            },
            active: {
              total: 4,
              http_requests: 1,
              sse_connections: 1,
              websocket_connections: 0,
              application_executions: 1,
              tool_calls: 0,
              model_requests: 1,
              waiting: null
            },
            peaks: {
              process_peak_concurrency: 8,
              recent_peak_concurrency: 5
            },
            rolling_minute: {
              completed: 12,
              failed: 1,
              cancelled: 0,
              disconnected: 2
            },
            windows: {
              one_minute: {
                window_seconds: 60,
                completed: 12,
                failed: 1,
                cancelled: 0,
                disconnected: 2,
                peak_concurrency: 5,
                failure_rate: 0.0769230769,
                disconnect_rate: 0.1333333333,
                throughput_per_minute: 12
              },
              five_minutes: {
                window_seconds: 300,
                completed: 40,
                failed: 2,
                cancelled: 0,
                disconnected: 3,
                peak_concurrency: 6,
                failure_rate: 0.0476190476,
                disconnect_rate: 0.0666666667,
                throughput_per_minute: 8
              },
              fifteen_minutes: {
                window_seconds: 900,
                completed: 90,
                failed: 3,
                cancelled: 0,
                disconnected: 5,
                peak_concurrency: 8,
                failure_rate: 0.0322580645,
                disconnect_rate: 0.0510204082,
                throughput_per_minute: 6
              }
            },
            health: {
              state: 'healthy',
              failure_rate_1m: 0.0769230769,
              failure_rate_5m: 0.0476190476,
              failure_rate_15m: 0.0322580645,
              disconnect_rate_5m: 0.0666666667,
              slow_ratio: 0,
              active_pressure: 0.8,
              throughput_5m_per_minute: 8,
              throughput_15m_per_minute: 6,
              throughput_trend: 'rising',
              failure_trend: 0.0153619831
            },
            age_distribution: {
              under_5s: 2,
              from_5s_to_30s: 1,
              from_30s_to_120s: 1,
              over_120s: 0
            },
            long_connection_age_distribution: {
              under_5s: 0,
              from_5s_to_30s: 1,
              from_30s_to_120s: 0,
              over_120s: 0
            },
            pressure: {
              slow_active_executions: 1,
              execution_slots_used: null,
              execution_slots_limit: null
            },
            resources: {
              process_rss_bytes: null
            }
          }
        }),
        { status: 200, headers: { 'content-type': 'application/json' } }
      )
    );

    await expect(
      getConsoleApplicationRuntimeActivity('app-1', 'http://127.0.0.1:7800')
    ).resolves.toMatchObject({
      meta: {
        scope: 'current_instance',
        storage: 'memory'
      },
      active: {
        total: 4,
        sse_connections: 1
      },
      rolling_minute: {
        disconnected: 2
      },
      health: {
        state: 'healthy'
      }
    });

    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/monitoring/runtime-activity',
      expect.objectContaining({ method: 'GET' })
    );
  });

  test('keeps application run log envelope fields from the logs routes', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          data: {
            items: [
              {
                id: 'run-1',
                application_id: 'app-1',
                application_type: 'agent_flow',
                run_object_kind: 'application_run',
                run_kind: 'debug_flow_run',
                run_mode: 'debug_flow_run',
                status: 'succeeded',
                target_node_id: null,
                title: '退款总结',
                source: 'console',
                compatibility_mode: null,
                subject: { kind: 'agent_flow', id: 'app-1' },
                actor: { kind: 'user', id: 'user-1', display_name: 'root' },
                correlation: {},
                statistics: {
                  count_tokens_input_tokens: null,
                  total_tokens: 50,
                  input_tokens: 40,
                  output_tokens: 10,
                  input_cache_hit_tokens: 12,
                  input_cache_hit_rate: 0.2307692308,
                  unique_node_count: 3,
                  tool_callback_count: 20
                },
                started_at: '2026-05-08T00:00:00Z',
                finished_at: null,
                created_at: '2026-05-08T00:00:00Z',
                updated_at: '2026-05-08T00:00:00Z'
              }
            ],
            total: 1,
            page: 1,
            page_size: 20
          }
        }),
        { status: 200, headers: { 'content-type': 'application/json' } }
      )
    );

    await expect(
      getConsoleApplicationRuns(
        'app-1',
        { cache_mode: 'refresh' },
        'http://127.0.0.1:7800'
      )
    ).resolves.toMatchObject({
      items: [
        {
          application_type: 'agent_flow',
          run_object_kind: 'application_run',
          compatibility_mode: null,
          subject: { kind: 'agent_flow' },
          statistics: {
            count_tokens_input_tokens: null,
            total_tokens: 50,
            input_tokens: 40,
            output_tokens: 10,
            input_cache_hit_tokens: 12,
            input_cache_hit_rate: 0.2307692308,
            unique_node_count: 3,
            tool_callback_count: 20
          }
        }
      ]
    });
    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/applications/app-1/logs/runs?page=1&page_size=20&cache_mode=refresh',
      expect.objectContaining({ method: 'GET' })
    );
  });
});
