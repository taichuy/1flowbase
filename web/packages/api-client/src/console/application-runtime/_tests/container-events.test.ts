import { expect, test } from 'vitest';
import { normalizeConsoleRuntimeEvent } from '../stream';

test('node lifecycle transport preserves canonical execution identity', () => {
  const execution_identity = {
    execution_id: 'execution',
    node_id: 'leaf',
    parent_execution_id: 'round',
    round_path: [
      {
        container_execution_id: 'container',
        container_node_id: 'loop',
        round_index: 2
      }
    ]
  };
  for (const event_type of ['node_started', 'node_finished']) {
    const event = normalizeConsoleRuntimeEvent({
      event_type,
      id: 'event',
      flow_run_id: 'flow',
      node_run_id: 'execution',
      sequence: 1,
      payload: {
        node_id: 'leaf',
        execution_identity,
        node_type: 'code',
        status: 'succeeded'
      }
    });
    expect(event).toEqual(
      expect.objectContaining({ type: event_type, execution_identity })
    );
  }
});
