import { describe, expect, test } from 'vitest';
import type { NodeExecutionIdentity } from '@1flowbase/api-client';
import { applyDebugStreamEventToTrace } from '../../lib/debug-console/stream-events';
import {
  containerRoundOptions,
  findContainerRound,
  isWithinContainerRound
} from '../../components/debug-console/trajectory/workflow/container-rounds';

function identity(round: number, inner?: number): NodeExecutionIdentity {
  return {
    execution_id: `execution-${round}-${inner ?? 'outer'}`,
    node_id: 'same-static-node',
    parent_execution_id: 'container',
    round_path: [
      {
        container_execution_id: 'outer',
        container_node_id: 'loop',
        round_index: round
      },
      ...(inner === undefined
        ? []
        : [
            {
              container_execution_id: `inner-${round}`,
              container_node_id: 'iteration',
              round_index: inner
            }
          ])
    ]
  };
}

describe('container execution identity consumers', () => {
  test('keeps repeated static node executions separate in live trace', () => {
    let trace = applyDebugStreamEventToTrace([], {
      type: 'node_started',
      node_run_id: 'run-0',
      node_id: 'same-static-node',
      node_type: 'template_transform',
      title: 'Item',
      execution_identity: identity(0)
    });
    trace = applyDebugStreamEventToTrace(trace, {
      type: 'node_started',
      node_run_id: 'run-1',
      node_id: 'same-static-node',
      node_type: 'template_transform',
      title: 'Item',
      execution_identity: identity(1)
    });
    trace = applyDebugStreamEventToTrace(trace, {
      type: 'node_finished',
      node_run_id: 'run-0',
      node_id: 'same-static-node',
      status: 'succeeded',
      execution_identity: identity(0),
      output_payload: { text: 'first' }
    });
    expect(trace).toHaveLength(2);
    expect(
      trace.map((item) => item.execution_identity?.round_path[0].round_index)
    ).toEqual([0, 1]);
    expect(trace[1].status).toBe('running');
    expect(trace[0].outputPayload).toEqual({ text: 'first' });
  });

  test('filters a parent round including its descendants without mixing sibling rounds', () => {
    const inputs = [
      identity(0, 0),
      identity(0, 1),
      identity(1, 0),
      identity(1, 1)
    ];
    const options = containerRoundOptions(
      inputs,
      new Map([['outer', 'Outer']]),
      'en-US'
    );
    expect(options).toHaveLength(2);
    expect(options[0].children).toHaveLength(2);
    const parent = findContainerRound(options, options[0].value);
    expect(
      inputs.filter((item) => isWithinContainerRound(item, parent))
    ).toEqual(inputs.slice(0, 2));
    const leaf = findContainerRound(options, options[1].children[1].value);
    expect(inputs.filter((item) => isWithinContainerRound(item, leaf))).toEqual(
      [inputs[3]]
    );
    expect(isWithinContainerRound(undefined, parent)).toBe(false);
    expect(isWithinContainerRound(undefined, [])).toBe(true);
  });
});
