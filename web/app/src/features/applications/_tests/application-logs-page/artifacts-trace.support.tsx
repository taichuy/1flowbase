import type { ConsoleApplicationRunDetail as ApplicationRunDetail } from '@1flowbase/api-client';
import { within } from '@testing-library/react';

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

export async function openLazyLlmNodeDetail(logPanel: HTMLElement) {
  const nodeDetail = await within(logPanel).findByRole('region', {
    name: 'LLM 节点详情'
  });
  expect(
    within(nodeDetail).queryByRole('button', { name: '详情' })
  ).not.toBeInTheDocument();

  return nodeDetail;
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

export function runOverviewFromDetail(detail: ApplicationRunDetail) {
  return {
    run: detail.run,
    statistics: detail.statistics ?? {
      total_tokens: null,
      input_tokens: null,
      output_tokens: null,
      input_cache_hit_tokens: null,
      unique_node_count: detail.node_runs.length,
      tool_callback_count: detail.callback_tasks.length
    },
    flow_run: detail.flow_run,
    answer_snapshot: detail.answer_snapshot ?? null
  };
}

export function traceRootNodeGroups(detail: ApplicationRunDetail) {
  const nodeRuns = [
    ...detail.node_runs,
    ...(detail.stitched_trace ?? []).flatMap((trace) => trace.node_runs)
  ];
  const groups: ApplicationRunDetail['node_runs'][] = [];
  const llmGroupIndexByNode = new Map<string, number>();

  for (const nodeRun of nodeRuns) {
    if (nodeRun.node_type !== 'llm') {
      groups.push([nodeRun]);
      continue;
    }

    const groupKey = `${nodeRun.flow_run_id}:${nodeRun.node_id}`;
    const groupIndex = llmGroupIndexByNode.get(groupKey);
    if (groupIndex !== undefined) {
      groups[groupIndex]!.push(nodeRun);
      continue;
    }

    llmGroupIndexByNode.set(groupKey, groups.length);
    groups.push([nodeRun]);
  }

  return groups;
}

export function traceNodeGroupId(nodeRuns: ApplicationRunDetail['node_runs']) {
  const firstNodeRun = nodeRuns[0]!;
  return nodeRuns.length > 1
    ? `node_run_group:${firstNodeRun.id}`
    : `node_run:${firstNodeRun.id}`;
}

export function mergeDebugPayloads(nodeRuns: ApplicationRunDetail['node_runs']) {
  const merged: Record<string, unknown> = {};
  const llmRounds: unknown[] = [];
  const routeTraces: unknown[] = [];
  const routeEvents: unknown[] = [];

  for (const nodeRun of nodeRuns) {
    const debugPayload = nodeRun.debug_payload ?? {};
    for (const [key, value] of Object.entries(debugPayload)) {
      if (key === 'llm_rounds') {
        if (Array.isArray(value)) {
          llmRounds.push(...value);
        } else if (merged.llm_rounds === undefined) {
          merged.llm_rounds = value;
        }
        continue;
      }
      if (key === 'visible_internal_llm_tool_trace') {
        if (Array.isArray(value)) {
          routeTraces.push(...value);
        } else if (merged.visible_internal_llm_tool_trace === undefined) {
          merged.visible_internal_llm_tool_trace = value;
        }
        continue;
      }
      if (key === 'visible_internal_llm_tool_events') {
        if (Array.isArray(value)) {
          routeEvents.push(...value);
        } else if (merged.visible_internal_llm_tool_events === undefined) {
          merged.visible_internal_llm_tool_events = value;
        }
        continue;
      }
      if (merged[key] === undefined) {
        merged[key] = value;
      }
    }
  }

  if (llmRounds.length > 0) {
    merged.llm_rounds = llmRounds;
  }
  if (routeTraces.length > 0) {
    merged.visible_internal_llm_tool_trace = routeTraces;
  }
  if (routeEvents.length > 0) {
    merged.visible_internal_llm_tool_events = routeEvents;
  }

  return merged;
}

export function payloadHasKeys(payload: Record<string, unknown>) {
  return Object.keys(payload).length > 0;
}

export function mergeNodeRunGroup(nodeRuns: ApplicationRunDetail['node_runs']) {
  const firstNodeRun = nodeRuns[0]!;
  const lastNodeRun = nodeRuns.at(-1) ?? firstNodeRun;

  if (nodeRuns.length === 1) {
    return firstNodeRun;
  }

  return {
    ...firstNodeRun,
    status: nodeRuns.some((nodeRun) => nodeRun.status === 'failed')
      ? 'failed'
      : nodeRuns.some((nodeRun) => nodeRun.status === 'waiting_callback')
        ? 'waiting_callback'
        : lastNodeRun.status,
    finished_at: nodeRuns.some((nodeRun) => nodeRun.finished_at === null)
      ? null
      : lastNodeRun.finished_at,
    input_payload:
      nodeRuns.find((nodeRun) => payloadHasKeys(nodeRun.input_payload))
        ?.input_payload ?? {},
    output_payload:
      [...nodeRuns]
        .reverse()
        .find((nodeRun) => payloadHasKeys(nodeRun.output_payload))
        ?.output_payload ?? {},
    error_payload:
      [...nodeRuns].reverse().find((nodeRun) => nodeRun.error_payload)
        ?.error_payload ?? null,
    metrics_payload:
      [...nodeRuns]
        .reverse()
        .find((nodeRun) => payloadHasKeys(nodeRun.metrics_payload))
        ?.metrics_payload ?? {},
    debug_payload: mergeDebugPayloads(nodeRuns)
  };
}

export function traceTreeFromDetail(detail: ApplicationRunDetail) {
  return {
    run: detail.run,
    statistics: detail.statistics,
    flow_run: detail.flow_run,
    answer_snapshot: detail.answer_snapshot ?? null,
    nodes: traceRootNodeGroups(detail).map((nodeRuns) => {
      const nodeRun = mergeNodeRunGroup(nodeRuns);

      return {
        trace_node_id: traceNodeGroupId(nodeRuns),
        parent_trace_node_id: null,
        node_kind: 'node_run',
        flow_run_id: nodeRun.flow_run_id,
        node_run_id: nodeRun.id,
        callback_task_id: null,
        node_id: nodeRun.node_id,
        node_type: nodeRun.node_type,
        node_alias: nodeRun.node_alias,
        status: nodeRun.status,
        started_at: nodeRun.started_at,
        finished_at: nodeRun.finished_at,
        duration_ms: null,
        metrics_payload: nodeRun.metrics_payload,
        has_children: false,
        has_content: true
      };
    })
  };
}

export function traceNodeRunGroupFromDetail(
  detail: ApplicationRunDetail,
  traceNodeId: string
) {
  const allNodeRuns = [
    ...detail.node_runs,
    ...(detail.stitched_trace ?? []).flatMap((trace) => trace.node_runs)
  ];

  if (traceNodeId.startsWith('node_run_group:')) {
    const firstNodeRunId = traceNodeId.slice('node_run_group:'.length);
    const firstNodeRun = allNodeRuns.find(
      (candidate) => candidate.id === firstNodeRunId
    );
    if (!firstNodeRun) {
      return [detail.node_runs[0]!];
    }

    return allNodeRuns.filter(
      (candidate) =>
        candidate.flow_run_id === firstNodeRun.flow_run_id &&
        candidate.node_id === firstNodeRun.node_id &&
        candidate.node_type === 'llm'
    );
  }

  const nodeRunId = traceNodeId.startsWith('node_run:')
    ? traceNodeId.slice('node_run:'.length)
    : traceNodeId;
  return [
    allNodeRuns.find((candidate) => candidate.id === nodeRunId) ??
      detail.node_runs[0]!
  ];
}

export function traceNodeContentFromDetail(
  detail: ApplicationRunDetail,
  traceNodeId: string
) {
  const nodeRunGroup = traceNodeRunGroupFromDetail(detail, traceNodeId);
  const nodeRun = mergeNodeRunGroup(nodeRunGroup);
  const nodeRunIds = new Set(nodeRunGroup.map((item) => item.id));
  const stitchedTrace =
    detail.stitched_trace?.find((trace) =>
      trace.node_runs.some((candidate) => nodeRunIds.has(candidate.id))
    ) ?? null;
  const checkpoints: ApplicationRunDetail['checkpoints'] = stitchedTrace
    ? []
    : detail.checkpoints;
  const events = stitchedTrace?.events ?? detail.events;

  return {
    trace_node_id: `node_run:${nodeRun.id}`,
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
      checkpoints: checkpoints.filter(
        (checkpoint) =>
          checkpoint.node_run_id !== null &&
          nodeRunIds.has(checkpoint.node_run_id)
      ),
      events: events.filter(
        (event) =>
          event.node_run_id !== null && nodeRunIds.has(event.node_run_id)
      )
    }
  };
}
