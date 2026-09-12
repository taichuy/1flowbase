import { describe, expect, test } from 'vitest';

import { isTraceGroupNode } from '../../components/debug-console/conversation-log-trace-model';

// #2035 AC-008: task rounds and child tasks reuse the group/lazy-children path.
describe('trace group nodes', () => {
  const base = {
    node_run_id: null,
    node_id: null,
    status: 'succeeded',
    started_at: '2026-09-12T00:00:00Z',
    finished_at: null,
    duration_ms: null,
    metrics_payload: {},
    has_children: true,
    child_count: 1,
    has_content: false
  };

  test('recognizes round groups, task rounds and child tasks as expandable groups', () => {
    expect(
      isTraceGroupNode({
        ...base,
        trace_node_id: 'rounds',
        node_kind: 'round_group',
        node_type: 'rounds',
        node_alias: 'Rounds'
      })
    ).toBe(true);
    expect(
      isTraceGroupNode({
        ...base,
        trace_node_id: 'round-2',
        node_kind: 'task_round',
        node_type: 'flow_run',
        node_alias: 'Round 2',
        source_flow_run_id: 'run-2'
      })
    ).toBe(true);
    expect(
      isTraceGroupNode({
        ...base,
        trace_node_id: 'child',
        node_kind: 'child_task',
        node_type: 'flow_run',
        node_alias: 'review · review diff',
        source_flow_run_id: 'run-child'
      })
    ).toBe(true);
    expect(
      isTraceGroupNode({
        ...base,
        trace_node_id: 'llm',
        node_kind: 'node_run',
        node_type: 'llm',
        node_alias: 'gpt'
      })
    ).toBe(false);
  });
});
