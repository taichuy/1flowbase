import type { ConsoleApplicationRunDetail as ApplicationRunDetail } from '@1flowbase/api-client';

export function applicationRunsPage<T>(
  items: T[],
  overrides?: Partial<{
    total: number;
    page: number;
    page_size: number;
  }>
) {
  return {
    items,
    total: overrides?.total ?? items.length,
    page: overrides?.page ?? 1,
    page_size: overrides?.page_size ?? 20
  };
}

export function conversationMessagesPage(
  items: Array<{
    id: string;
    flow_run_id: string | null;
    role: 'system' | 'user' | 'assistant';
    content: string;
    sequence: number;
    status?: string;
    started_at?: string | null;
    finished_at?: string | null;
  }>
) {
  return {
    items: items.map((item) => ({
      run_id: item.flow_run_id ?? `message:${item.id}`,
      detail_run_id: item.flow_run_id,
      can_open_detail: Boolean(item.flow_run_id),
      role: item.role,
      content: item.content,
      started_at: item.started_at ?? '2026-04-17T09:00:00Z',
      finished_at: item.finished_at ?? '2026-04-17T09:00:01Z',
      status: item.status ?? 'succeeded',
      query: null,
      model: null,
      answer: null,
      is_current: item.flow_run_id === 'run-1'
    })),
    page: {
      has_before: false,
      has_after: false,
      before_cursor: null,
      after_cursor: null
    }
  };
}

export function lastElement<T>(items: T[], message: string): T {
  const item = items.at(-1);
  if (!item) {
    throw new Error(message);
  }
  return item;
}

export function sampleRunDetail(): ApplicationRunDetail {
  return {
    run: {
      id: 'run-1',
      application_id: 'app-1',
      application_type: 'agent_flow',
      run_object_kind: 'flow_run',
      run_kind: 'published_api_run',
      status: 'succeeded',
      title: '公开 API 退款总结',
      execution_stage: 'published',
      invocation_source: 'agent_flow_api',
      compatibility_mode: 'openai-responses-v1',
      subject: {
        kind: 'agent_flow',
        id: 'flow-1',
        draft_id: 'draft-1',
        target_node_id: 'node-llm'
      },
      principal: {
        kind: 'application_api_key',
        id: 'key-1',
        display_name: null
      },
      correlation: {
        compatibility_mode: 'openai-responses-v1'
      },
      started_at: '2026-04-17T09:00:00Z',
      finished_at: '2026-04-17T09:00:01Z',
      created_at: '2026-04-17T09:00:00Z',
      updated_at: '2026-04-17T09:00:01Z'
    },
    statistics: {
      count_tokens_input_tokens: null,
      total_tokens: 50,
      input_tokens: 40,
      output_tokens: 10,
      input_cache_hit_tokens: 12,
      input_cache_hit_rate: null,
      unique_node_count: 3,
      tool_callback_count: 20
    },
    flow_run: {
      id: 'run-1',
      application_id: 'app-1',
      flow_id: 'flow-1',
      draft_id: 'draft-1',
      compiled_plan_id: 'plan-1',
      run_mode: 'published_api_run' as const,
      status: 'succeeded',
      target_node_id: 'node-llm',
      title: '公开 API 退款总结',
      expand_id: 'customer-42',
      authorized_account: 'root',
      external_conversation_id: 'conversation-1',
      query: '总结退款政策',
      model: 'deepseek-chat',
      input_payload: {
        __runtime_debug_artifact: true,
        artifact_ref: 'artifact-flow-input',
        content_type: 'application/json',
        is_truncated: true,
        original_size_bytes: 54538,
        preview_size_bytes: 2048,
        preview:
          '{"node-start":{"compatibility":{"tools":[{"function":{"description":"path to the file to read."}}]}}}'
      } as Record<string, unknown>,
      output_payload: {
        answer: '退款政策摘要',
        resolved_inputs: {
          user_prompt: '总结退款政策'
        }
      },
      error_payload: null,
      created_by: 'user-1',
      started_at: '2026-04-17T09:00:00Z',
      finished_at: '2026-04-17T09:00:01Z',
      created_at: '2026-04-17T09:00:00Z',
      updated_at: '2026-04-17T09:00:01Z'
    },
    node_runs: [
      {
        id: 'node-run-1',
        flow_run_id: 'run-1',
        node_id: 'node-llm',
        node_type: 'llm',
        node_alias: 'LLM',
        status: 'succeeded',
        input_payload: {
          user_prompt: '总结退款政策'
        },
        output_payload: {
          answer: '退款政策摘要',
          rendered_templates: {}
        },
        error_payload: null,
        metrics_payload: {
          output_contract_count: 1
        },
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z'
      }
    ],
    checkpoints: [],
    callback_tasks: [],
    events: [
      {
        id: 'event-1',
        flow_run_id: 'run-1',
        node_run_id: 'node-run-1',
        sequence: 1,
        event_type: 'node_preview_started',
        payload: {
          target_node_id: 'node-llm'
        },
        created_at: '2026-04-17T09:00:00Z'
      },
      {
        id: 'event-2',
        flow_run_id: 'run-1',
        node_run_id: 'node-run-1',
        sequence: 2,
        event_type: 'node_preview_completed',
        payload: {
          target_node_id: 'node-llm'
        },
        created_at: '2026-04-17T09:00:01Z'
      }
    ]
  };
}

export function sampleTraceTree() {
  return {
    run: {
      id: 'run-1',
      application_id: 'app-1',
      application_type: 'agent_flow',
      run_object_kind: 'application_run',
      run_kind: 'published_api_run',
      status: 'succeeded',
      title: '公开 API 退款总结',
      execution_stage: 'published',
      invocation_source: 'agent_flow_api',
      compatibility_mode: 'openai-responses-v1',
      subject: {
        kind: 'agent_flow',
        id: 'flow-1',
        draft_id: 'draft-1',
        target_node_id: 'node-llm'
      },
      principal: {
        kind: 'application_api_key',
        id: 'key-1',
        display_name: null
      },
      correlation: {
        compatibility_mode: 'openai-responses-v1'
      },
      started_at: '2026-04-17T09:00:00Z',
      finished_at: '2026-04-17T09:00:01Z',
      created_at: '2026-04-17T09:00:00Z',
      updated_at: '2026-04-17T09:00:01Z'
    },
    statistics: {
      total_tokens: 50,
      input_tokens: 40,
      output_tokens: 10,
      input_cache_hit_tokens: 12,
      input_cache_hit_rate: null,
      unique_node_count: 3,
      tool_callback_count: 20
    },
    flow_run: sampleRunDetail().flow_run,
    answer_snapshot: null,
    nodes: [
      {
        trace_node_id: 'node_run:node-run-1',
        parent_trace_node_id: null,
        node_kind: 'node_run',
        flow_run_id: 'run-1',
        node_run_id: 'node-run-1',
        node_id: 'node-llm',
        node_type: 'llm',
        node_alias: 'LLM',
        status: 'succeeded',
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z',
        duration_ms: 1000,
        metrics_payload: {
          output_contract_count: 1
        },
        has_children: false,
        has_content: true
      }
    ]
  };
}

export function sampleRunOverview() {
  const detail = sampleRunDetail();

  return {
    run: detail.run,
    statistics: detail.statistics,
    flow_run: detail.flow_run,
    answer_snapshot: detail.answer_snapshot ?? null
  };
}

export function sampleTraceNodeContent() {
  const detail = sampleRunDetail();
  const nodeRun = detail.node_runs[0];

  return {
    trace_node_id: 'node_run:node-run-1',
    node_kind: 'node_run',
    content_kind: 'node_run',
    source_refs: [],
    detail_refs: [],
    payload: {
      input_payload: nodeRun.input_payload,
      output_payload: nodeRun.output_payload,
      error_payload: nodeRun.error_payload,
      metrics_payload: nodeRun.metrics_payload,
      debug_payload: nodeRun.debug_payload,
      events: detail.events
    }
  };
}
