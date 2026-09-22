import type { Mock } from 'vitest';
import type { ConversationLogTraceNodeSummary } from '../../../../agent-flow/components/debug-console/conversation-log-trace-model';

// Frozen backend read DTOs: each LLM execution owns a distinct callback subtree.
export function configureExecutionProjection(
  api: {
    fetchApplicationRunTraceTree: Mock;
    fetchApplicationRunTraceNodeChildren: Mock;
    fetchApplicationRunTraceNodeContent: Mock;
    fetchApplicationRunTraceNodeDetail: Mock;
  },
  options: {
    mode?: 'route' | 'fusion';
    repeated?: boolean;
    sourceRunId?: string;
    artifact?: boolean;
  } = {}
) {
  const mode = options.mode ?? 'route';
  const sourceRunId = options.sourceRunId ?? 'run-1';
  const nodes = new Map<string, ConversationLogTraceNodeSummary>();
  const children = new Map<string, ConversationLogTraceNodeSummary[]>();
  const payloads = new Map<string, Record<string, unknown>>();
  function node(
    id: string,
    alias: string,
    kind: string,
    type = kind
  ): ConversationLogTraceNodeSummary {
    const value = {
      trace_node_id: id,
      node_kind: kind,
      node_type: type,
      node_alias: alias,
      flow_run_id: 'run-1',
      source_flow_run_id: sourceRunId === 'run-1' ? null : sourceRunId,
      node_id: type === 'llm' ? 'same-llm-node' : null,
      node_run_id: kind === 'node_run' ? id : null,
      status: 'succeeded',
      started_at: '2026-04-17T09:00:00Z',
      finished_at: '2026-04-17T09:00:01Z',
      duration_ms: 1000,
      has_children: false,
      has_content: kind !== 'tool_group',
      metrics_payload: {}
    };
    nodes.set(id, value);
    return value;
  }
  function attach(
    parent: ConversationLogTraceNodeSummary,
    list: ConversationLogTraceNodeSummary[]
  ) {
    parent.has_children = list.length > 0;
    parent.child_count = list.length;
    children.set(parent.trace_node_id, list);
  }
  const roots = [node('execution-1', 'LLM', 'node_run', 'llm')];
  if (options.repeated)
    roots.push(node('execution-2', 'LLM', 'node_run', 'llm'));
  roots.forEach((root, index) => {
    const suffix = String(index + 1);
    const group = node(`tools-${suffix}`, 'Tools', 'tool_group', 'tools');
    const tool = node(
      `callback-${suffix}`,
      index ? 'read_policy' : 'lookup_weather',
      'tool_callback',
      'tool'
    );
    tool.node_mode = mode;
    const route = node(`mode-${suffix}`, mode, mode);
    const branches = [
      node(
        `branch-${suffix}-1`,
        mode === 'fusion' ? 'Risk Panel' : 'Image LLM',
        'branch',
        'llm'
      )
    ];
    if (mode === 'fusion')
      branches.push(
        node(`branch-${suffix}-2`, 'Support Panel', 'branch', 'llm')
      );
    attach(root, [group]);
    attach(group, [tool]);
    attach(tool, [route]);
    attach(route, branches);
    payloads.set(root.trace_node_id, {
      input_payload: { prompt: `execution ${suffix}` },
      debug_payload: {},
      output_payload: { answer: `answer ${suffix}` }
    });
    payloads.set(tool.trace_node_id, {
      id: `call-${suffix}`,
      name: tool.node_alias,
      callback_status: 'returned',
      execution_status: 'succeeded',
      request_payload: { city: 'Shanghai' },
      callback_payload: { temperature: 'warm' },
      parsed_result: { temperature: 'warm' }
    });
    for (const branch of branches)
      payloads.set(branch.trace_node_id, {
        input_payload: { prompt: `${branch.node_alias} input` },
        debug_payload: {
          provider: 'recorded-provider',
          llm_rounds: [
            {
              round_index: 0,
              assistant: {
                role: 'assistant',
                tool_calls: [
                  {
                    id: 'nested-lookup',
                    name: 'branch_policy_lookup',
                    arguments: { topic: 'refund' }
                  }
                ]
              }
            },
            {
              round_index: 1,
              tool_results: [
                {
                  tool_call_id: 'nested-lookup',
                  name: 'branch_policy_lookup',
                  content: 'branch policy result'
                }
              ]
            }
          ]
        },
        output_payload: options.artifact
          ? {
              __runtime_debug_artifact: true,
              artifact_ref: 'artifact-branch-output',
              kind: 'node_output',
              preview: 'branch output preview'
            }
          : { text: `${branch.node_alias} result` }
      });
  });
  api.fetchApplicationRunTraceTree.mockResolvedValue({
    nodes: roots,
    page_info: { has_more: false, next_cursor: null, page_size: 20 }
  });
  api.fetchApplicationRunTraceNodeChildren.mockImplementation(
    async (_app: string, _run: string, parent: string) => ({
      items: children.get(parent) ?? [],
      page_info: { has_more: false, next_cursor: null, page_size: 20 }
    })
  );
  api.fetchApplicationRunTraceNodeContent.mockImplementation(
    async (_app: string, _run: string, id: string) => {
      const value = nodes.get(id)!;
      return {
        trace_node_id: id,
        node_kind: value.node_kind,
        content_kind: value.node_kind,
        detail_refs:
          value.node_kind === 'node_run'
            ? [{ detail_ref_id: 'node_run', detail_kind: 'node_run' }]
            : [],
        payload:
          value.node_kind === 'tool_callback' || value.node_kind === 'branch'
            ? payloads.get(id)
            : { payload_index: { node_run_count: 1 } }
      };
    }
  );
  api.fetchApplicationRunTraceNodeDetail.mockImplementation(
    async (
      _app: string,
      _run: string,
      id: string,
      detailRefId: string,
      section: string
    ) => ({
      trace_node_id: id,
      node_kind: nodes.get(id)!.node_kind,
      detail_ref_id: detailRefId,
      detail_kind: 'node_run',
      payload: { node_run: { [section]: payloads.get(id)![section] } }
    })
  );
  return { roots, nodes };
}
