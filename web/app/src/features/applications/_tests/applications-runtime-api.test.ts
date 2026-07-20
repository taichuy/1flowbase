import { afterEach, describe, expect, test, vi } from 'vitest';

const traceProjectionStatus = vi.hoisted(() => ({
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
}));

vi.mock('@1flowbase/api-client', () => ({
  completeConsoleCallbackTask: vi.fn().mockResolvedValue(undefined),
  completeConsoleRunArchiveUploadSession: vi.fn().mockResolvedValue({
    job_id: 'job-1',
    status: 'queued'
  }),
  createConsoleRunArchiveUploadSession: vi.fn().mockResolvedValue({
    session_id: 'session-1',
    status: 'uploading'
  }),
  getConsoleApplicationRunTraceTree: vi.fn().mockResolvedValue({
    projection_status: traceProjectionStatus,
    nodes: []
  }),
  getConsoleApplicationRunTraceNodeChildren: vi.fn().mockResolvedValue({
    projection_status: traceProjectionStatus,
    items: [],
    page_info: {
      has_more: false,
      next_cursor: null,
      page_size: 20
    }
  }),
  getConsoleApplicationRunTraceNodeContent: vi.fn().mockResolvedValue({
    trace_node_id: '11111111-1111-4111-8111-111111111111',
    node_kind: 'node_run',
    projection_status: traceProjectionStatus,
    content_kind: 'node_run',
    source_refs: [],
    detail_refs: [],
    payload: {}
  }),
  getConsoleApplicationRunTraceNodeDetail: vi.fn().mockResolvedValue({
    trace_node_id: '11111111-1111-4111-8111-111111111111',
    node_kind: 'node_run',
    projection_status: traceProjectionStatus,
    detail_ref_id: 'node_run',
    detail_kind: 'node_run',
    source_refs: [],
    payload: {}
  }),
  exportConsoleApplicationRunTraceDump: vi.fn().mockResolvedValue({
    blob: new Blob(['{}'], { type: 'application/json' }),
    filename: 'run-1.json',
    contentType: 'application/json'
  }),
  exportConsoleApplicationRunsTraceDumpZip: vi.fn().mockResolvedValue({
    blob: new Blob(['zip'], { type: 'application/zip' }),
    filename: 'selected-runs.zip',
    contentType: 'application/zip'
  }),
  getConsoleApplicationRunResumeTimeline: vi.fn().mockResolvedValue({
    flow_run: { id: 'run-1' },
    callback_tasks: [],
    events: []
  }),
  getConsoleApplicationRunMonitoringReport: vi.fn().mockResolvedValue({
    meta: {
      started_from: null,
      started_to: null,
      bucket: 'day',
      slow_run_threshold_ms: 30000
    },
    overview: {
      total_count: 0,
      success_count: 0,
      failed_count: 0,
      cancelled_count: 0,
      success_rate: 0,
      failed_rate: 0,
      running_count_included: false
    },
    duration: {
      duration_recorded_count: 0,
      avg_duration_ms: 0,
      p50_duration_ms: 0,
      p95_duration_ms: 0,
      slow_run_rate: 0
    },
    tokens: {
      total_tokens_sum: 0,
      input_tokens_sum: 0,
      output_tokens_sum: 0,
      input_cache_hit_tokens_sum: 0,
      avg_tokens_per_run: 0,
      token_recorded_count: 0
    },
    tokens_comparison: {
      previous_total_tokens_sum: 0,
      previous_run_count: 0,
      previous_avg_tokens_per_run: 0,
      token_change_rate: 0,
      run_count_change_rate: 0,
      avg_tokens_per_run_change_rate: 0,
      traffic_effect: 0,
      cost_per_run_effect: 0
    },
    tool_callbacks: {
      total_tool_callback_count: 0,
      avg_tool_callback_count: 0,
      runs_with_tool_callback: 0
    },
    nodes: {
      avg_unique_node_count: 0,
      max_unique_node_count: 0
    },
    concurrency: {
      peak_concurrency: 0
    },
    tokens_trend: [],
    protocols: [],
    sources: [],
    authorized_accounts: [],
    external_users: [],
    api_keys: [],
    external_conversations: [],
    slowest_runs: [],
    high_token_runs: []
  }),
  getConsoleApplicationRuntimeActivity: vi.fn().mockResolvedValue({
    meta: {
      application_id: 'app-1',
      scope: 'current_instance',
      storage: 'memory',
      instance_started_at: '2026-05-30T00:00:00Z',
      snapshot_at: '2026-05-30T00:01:00Z'
    },
    active: {
      total: 0,
      http_requests: 0,
      sse_connections: 0,
      websocket_connections: 0,
      application_executions: 0,
      tool_calls: 0,
      model_requests: 0,
      waiting: null
    },
    peaks: {
      process_peak_concurrency: 0,
      recent_peak_concurrency: 0
    },
    rolling_minute: {
      completed: 0,
      failed: 0,
      cancelled: 0,
      disconnected: 0
    },
    windows: {
      one_minute: {
        window_seconds: 60,
        completed: 0,
        failed: 0,
        cancelled: 0,
        disconnected: 0,
        peak_concurrency: 0,
        failure_rate: 0,
        disconnect_rate: 0,
        throughput_per_minute: 0
      },
      five_minutes: {
        window_seconds: 300,
        completed: 0,
        failed: 0,
        cancelled: 0,
        disconnected: 0,
        peak_concurrency: 0,
        failure_rate: 0,
        disconnect_rate: 0,
        throughput_per_minute: 0
      },
      fifteen_minutes: {
        window_seconds: 900,
        completed: 0,
        failed: 0,
        cancelled: 0,
        disconnected: 0,
        peak_concurrency: 0,
        failure_rate: 0,
        disconnect_rate: 0,
        throughput_per_minute: 0
      }
    },
    health: {
      state: 'healthy',
      failure_rate_1m: 0,
      failure_rate_5m: 0,
      failure_rate_15m: 0,
      disconnect_rate_5m: 0,
      slow_ratio: 0,
      active_pressure: 0,
      throughput_5m_per_minute: 0,
      throughput_15m_per_minute: 0,
      throughput_trend: 'steady',
      failure_trend: 0
    },
    age_distribution: {
      under_5s: 0,
      from_5s_to_30s: 0,
      from_30s_to_120s: 0,
      over_120s: 0
    },
    long_connection_age_distribution: {
      under_5s: 0,
      from_5s_to_30s: 0,
      from_30s_to_120s: 0,
      over_120s: 0
    },
    pressure: {
      slow_active_executions: 0,
      execution_slots_used: null,
      execution_slots_limit: null
    },
    resources: {
      process_rss_bytes: null
    }
  }),
  getConsoleApplicationRuns: vi.fn().mockResolvedValue([]),
  fetchConsoleRuntimeModelRecords: vi.fn().mockResolvedValue({
    items: [],
    total: 0
  }),
  getConsoleRuntimeDebugStream: vi.fn().mockResolvedValue({
    parts: [],
    page_size: 500,
    next_sequence: null,
    has_more: false
  }),
  getConsoleRunArchiveImportJob: vi.fn().mockResolvedValue({
    job_id: 'job-1',
    status: 'succeeded',
    imported_run_count: 1
  }),
  resumeConsoleFlowRun: vi.fn().mockResolvedValue(undefined),
  uploadConsoleRunArchiveChunk: vi.fn().mockResolvedValue({
    session_id: 'session-1',
    chunk_index: 0
  }),
  getDefaultApiBaseUrl: vi.fn().mockReturnValue('http://127.0.0.1:7800')
}));

import {
  completeConsoleCallbackTask,
  completeConsoleRunArchiveUploadSession,
  createConsoleRunArchiveUploadSession,
  fetchConsoleRuntimeModelRecords,
  getConsoleApplicationRunMonitoringReport,
  getConsoleApplicationRunResumeTimeline,
  getConsoleApplicationRunTraceNodeChildren,
  getConsoleApplicationRunTraceNodeContent,
  getConsoleApplicationRunTraceNodeDetail,
  getConsoleApplicationRunTraceTree,
  getConsoleApplicationRuntimeActivity,
  getConsoleApplicationRuns,
  getConsoleRuntimeDebugStream,
  getConsoleRunArchiveImportJob,
  exportConsoleApplicationRunTraceDump,
  exportConsoleApplicationRunsTraceDumpZip,
  resumeConsoleFlowRun,
  uploadConsoleRunArchiveChunk
} from '@1flowbase/api-client';

import {
  applicationRunMonitoringReportQueryKey,
  applicationRunResumeTimelineQueryKey,
  applicationRunTraceNodeChildrenQueryKey,
  applicationRunTraceNodeContentQueryKey,
  applicationRunTraceTreeQueryKey,
  applicationRuntimeActivityQueryKey,
  applicationRunsQueryKey,
  applicationRuntimeDebugStreamQueryKey,
  completeCallbackTask,
  completeApplicationRunArchiveUploadSession,
  createApplicationRunArchiveUploadSession,
  fetchApplicationConversationMessages,
  fetchApplicationRunArchiveImportJob,
  fetchApplicationRunMonitoringReport,
  fetchApplicationRunResumeTimeline,
  fetchApplicationRunTraceNodeChildren,
  fetchApplicationRunTraceNodeContent,
  fetchApplicationRunTraceNodeDetail,
  fetchApplicationRunTraceTree,
  fetchApplicationRuntimeActivity,
  fetchApplicationRuns,
  exportApplicationRunTraceDump,
  exportSelectedApplicationRunsTraceDumpZip,
  fetchRuntimeDebugStream,
  resumeFlowRun,
  uploadApplicationRunArchiveChunk
} from '../api/runtime';

afterEach(() => {
  vi.clearAllMocks();
});

describe('applications runtime api', () => {
  test('builds stable runtime query keys', () => {
    const traceNodeId = '11111111-1111-4111-8111-111111111111';

    expect(applicationRunsQueryKey('app-1')).toEqual([
      'applications',
      'app-1',
      'runtime',
      'runs',
      1,
      20,
      'all',
      'started_at',
      'desc',
      ''
    ]);
    expect(applicationRunsQueryKey('app-1', { titleIncludes: '退款' })).toEqual(
      [
        'applications',
        'app-1',
        'runtime',
        'runs',
        1,
        20,
        'all',
        'started_at',
        'desc',
        '退款'
      ]
    );
    expect(applicationRunTraceTreeQueryKey('app-1', 'run-1')).toEqual([
      'applications',
      'app-1',
      'runtime',
      'runs',
      'run-1',
      'trace-tree'
    ]);
    expect(
      applicationRunTraceNodeChildrenQueryKey('app-1', 'run-1', traceNodeId)
    ).toEqual([
      'applications',
      'app-1',
      'runtime',
      'runs',
      'run-1',
      'trace-tree',
      traceNodeId,
      'children'
    ]);
    expect(
      applicationRunTraceNodeContentQueryKey('app-1', 'run-1', traceNodeId)
    ).toEqual([
      'applications',
      'app-1',
      'runtime',
      'runs',
      'run-1',
      'trace-tree',
      traceNodeId,
      'content'
    ]);
    expect(applicationRunResumeTimelineQueryKey('app-1', 'run-1')).toEqual([
      'applications',
      'app-1',
      'runtime',
      'runs',
      'run-1',
      'resume-timeline'
    ]);
    expect(applicationRuntimeDebugStreamQueryKey('app-1', 'run-1')).toEqual([
      'applications',
      'app-1',
      'runtime',
      'runs',
      'run-1',
      'debug-stream'
    ]);
    expect(applicationRunMonitoringReportQueryKey('app-1')).toEqual([
      'applications',
      'app-1',
      'runtime',
      'monitoring',
      'run-metrics',
      7,
      'day'
    ]);
    expect(applicationRuntimeActivityQueryKey('app-1')).toEqual([
      'applications',
      'app-1',
      'runtime',
      'monitoring',
      'runtime-activity'
    ]);
  });

  test('passes the resolved base url to runtime read requests', async () => {
    const traceNodeId = '11111111-1111-4111-8111-111111111111';

    await fetchApplicationRuns('app-1', { cacheMode: 'refresh' });
    await fetchApplicationRunTraceTree('app-1', 'run-1');
    await fetchApplicationRunTraceNodeChildren('app-1', 'run-1', traceNodeId);
    await fetchApplicationRunTraceNodeContent('app-1', 'run-1', traceNodeId);
    await fetchApplicationRunTraceNodeDetail(
      'app-1',
      'run-1',
      traceNodeId,
      'node_run'
    );
    await fetchApplicationRunResumeTimeline('app-1', 'run-1');
    await fetchApplicationRunMonitoringReport('app-1', {
      timeRangeDays: 28,
      bucket: 'week'
    });
    await fetchApplicationRuntimeActivity('app-1');
    await fetchRuntimeDebugStream('app-1', 'run-1');
    await exportApplicationRunTraceDump('app-1', 'run-1');
    await exportSelectedApplicationRunsTraceDumpZip(
      'app-1',
      ['run-1', 'run-2'],
      'csrf-123'
    );
    await createApplicationRunArchiveUploadSession(
      'app-1',
      {
        filename: 'archive.json',
        total_size_bytes: 42,
        expected_sha256: 'sha256:archive',
        chunk_size_bytes: 1024
      },
      'csrf-123'
    );
    const chunk = new Blob(['chunk']);
    await uploadApplicationRunArchiveChunk(
      'app-1',
      'session-1',
      0,
      chunk,
      'sha256:chunk',
      'csrf-123'
    );
    await completeApplicationRunArchiveUploadSession(
      'app-1',
      'session-1',
      'csrf-123'
    );
    await fetchApplicationRunArchiveImportJob('app-1', 'job-1');

    expect(fetchConsoleRuntimeModelRecords).toHaveBeenCalledWith(
      'application_run_log_summaries',
      expect.objectContaining({
        page: 1,
        page_size: 20,
        sort: {
          field: 'started_at',
          direction: 'desc'
        }
      }),
      'http://127.0.0.1:7800'
    );
    expect(getConsoleApplicationRuns).not.toHaveBeenCalled();
    expect(getConsoleApplicationRunTraceTree).toHaveBeenCalledWith(
      'app-1',
      'run-1',
      'http://127.0.0.1:7800'
    );
    expect(getConsoleApplicationRunTraceNodeChildren).toHaveBeenCalledWith(
      'app-1',
      'run-1',
      traceNodeId,
      'http://127.0.0.1:7800',
      undefined
    );
    expect(getConsoleApplicationRunTraceNodeContent).toHaveBeenCalledWith(
      'app-1',
      'run-1',
      traceNodeId,
      'http://127.0.0.1:7800',
      {
        artifact_preview: 'auto'
      }
    );
    expect(getConsoleApplicationRunTraceNodeDetail).toHaveBeenCalledWith(
      'app-1',
      'run-1',
      traceNodeId,
      'node_run',
      'http://127.0.0.1:7800',
      {
        artifact_preview: 'auto'
      }
    );
    expect(getConsoleApplicationRunResumeTimeline).toHaveBeenCalledWith(
      'app-1',
      'run-1',
      'http://127.0.0.1:7800'
    );
    expect(getConsoleApplicationRunMonitoringReport).toHaveBeenCalledWith(
      'app-1',
      {
        time_range_days: 28,
        bucket: 'week'
      },
      'http://127.0.0.1:7800'
    );
    expect(getConsoleApplicationRuntimeActivity).toHaveBeenCalledWith(
      'app-1',
      'http://127.0.0.1:7800'
    );
    expect(getConsoleRuntimeDebugStream).toHaveBeenCalledWith(
      'app-1',
      'run-1',
      'http://127.0.0.1:7800'
    );
    expect(exportConsoleApplicationRunTraceDump).toHaveBeenCalledWith(
      'app-1',
      'run-1',
      'http://127.0.0.1:7800'
    );
    expect(exportConsoleApplicationRunsTraceDumpZip).toHaveBeenCalledWith(
      'app-1',
      ['run-1', 'run-2'],
      'csrf-123',
      'http://127.0.0.1:7800'
    );
    expect(createConsoleRunArchiveUploadSession).toHaveBeenCalledWith(
      'app-1',
      {
        filename: 'archive.json',
        total_size_bytes: 42,
        expected_sha256: 'sha256:archive',
        chunk_size_bytes: 1024
      },
      'csrf-123',
      'http://127.0.0.1:7800'
    );
    expect(uploadConsoleRunArchiveChunk).toHaveBeenCalledWith(
      'app-1',
      'session-1',
      0,
      chunk,
      'sha256:chunk',
      'csrf-123',
      'http://127.0.0.1:7800'
    );
    expect(completeConsoleRunArchiveUploadSession).toHaveBeenCalledWith(
      'app-1',
      'session-1',
      'csrf-123',
      'http://127.0.0.1:7800'
    );
    expect(getConsoleRunArchiveImportJob).toHaveBeenCalledWith(
      'app-1',
      'job-1',
      'http://127.0.0.1:7800'
    );
  });

  test('reads run log summaries from Runtime Data Model records', async () => {
    vi.mocked(fetchConsoleRuntimeModelRecords).mockResolvedValueOnce({
      items: [
        {
          id: 'run-1',
          application_id: 'app-1',
          scope_id: 'workspace-1',
          run_mode: 'published_api_run',
          execution_stage: 'published',
          invocation_source: 'agent_flow_api',
          principal: {
            kind: 'application_api_key',
            id: 'key-1',
            display_name: null
          },
          status: 'succeeded',
          target_node_id: null,
          title: '退款总结',
          execution_stage: 'published',
          invocation_source: 'agent_flow_api',
          principal: {
            kind: 'application_api_key',
            id: 'key-1',
            display_name: null
          },
          external_conversation_id: 'conversation-1',
          total_tokens: 120,
          input_tokens: 90,
          output_tokens: 30,
          input_cache_hit_tokens: 45,
          unique_node_count: 3,
          tool_callback_count: 1,
          started_at: '2026-05-08T00:00:00Z',
          finished_at: '2026-05-08T00:00:01Z',
          created_at: '2026-05-08T00:00:00Z',
          updated_at: '2026-05-08T00:00:01Z'
        }
      ],
      total: 1
    });

    const runsPage = await fetchApplicationRuns('app-1', {
      titleIncludes: '退款'
    });

    expect(runsPage).toMatchObject({
      items: [
        {
          id: 'run-1',
          application_id: 'app-1',
          scope_id: 'workspace-1',
          title: '退款总结',
          total_tokens: 120,
          input_tokens: 90,
          output_tokens: 30,
          input_cache_hit_tokens: 45
        }
      ],
      total: 1,
      page: 1,
      page_size: 20
    });
    expect(runsPage.items[0]).not.toHaveProperty('flow_run_id');

    expect(fetchConsoleRuntimeModelRecords).toHaveBeenCalledWith(
      'application_run_log_summaries',
      {
        page: 1,
        page_size: 20,
        filter: {
          application_id: { $eq: 'app-1' },
          title: { $includes: '退款' }
        },
        sort: {
          field: 'started_at',
          direction: 'desc'
        }
      },
      'http://127.0.0.1:7800'
    );
    expect(getConsoleApplicationRuns).not.toHaveBeenCalled();
  });

  test('reads conversation messages from Runtime Data Model records', async () => {
    vi.mocked(fetchConsoleRuntimeModelRecords).mockResolvedValueOnce({
      items: [
        {
          id: 'message-1',
          application_id: 'app-1',
          scope_id: 'workspace-1',
          conversation_id: 'conversation-record-1',
          flow_run_id: 'run-1',
          role: 'assistant',
          content: '退款政策摘要',
          sequence: 2,
          status: 'succeeded',
          created_at: '2026-05-08T00:00:01Z'
        }
      ],
      total: 1
    });

    await expect(
      fetchApplicationConversationMessages('app-1', {
        conversationId: 'conversation-record-1',
        page: 1,
        pageSize: 5
      })
    ).resolves.toMatchObject({
      items: [
        {
          id: 'message-1',
          conversation_id: 'conversation-record-1',
          flow_run_id: 'run-1',
          role: 'assistant',
          content: '退款政策摘要'
        }
      ],
      total: 1,
      page: 1,
      page_size: 5
    });

    expect(fetchConsoleRuntimeModelRecords).toHaveBeenCalledWith(
      'application_conversation_messages',
      {
        page: 1,
        page_size: 5,
        filter: {
          application_id: { $eq: 'app-1' },
          conversation_id: { $eq: 'conversation-record-1' }
        },
        sort: {
          field: 'sequence',
          direction: 'desc'
        }
      },
      'http://127.0.0.1:7800'
    );
  });

  test('maps runtime mutation payloads before calling the console client', async () => {
    await resumeFlowRun(
      'app-1',
      'run-1',
      'checkpoint-1',
      { approved: true },
      'csrf-123'
    );
    await completeCallbackTask(
      'app-1',
      'callback-1',
      { decision: 'approve' },
      'csrf-123'
    );

    expect(resumeConsoleFlowRun).toHaveBeenCalledWith(
      'app-1',
      'run-1',
      {
        checkpoint_id: 'checkpoint-1',
        input_payload: { approved: true }
      },
      'csrf-123',
      'http://127.0.0.1:7800'
    );
    expect(completeConsoleCallbackTask).toHaveBeenCalledWith(
      'app-1',
      'callback-1',
      {
        response_payload: { decision: 'approve' }
      },
      'csrf-123',
      'http://127.0.0.1:7800'
    );
  });
});
