import { describe, expect, test } from 'vitest';
import {
  applyWorkflowRunEvent,
  emptyWorkflowRunState,
  isWorkflowRunActive,
  workflowNodeRunStatuses
} from '../lib/run-state';

const start = (node_id: string, node_run_id = node_id) => ({
  type: 'node_started' as const,
  node_id,
  node_run_id,
  node_type: 'template_transform',
  title: node_id
});
const finish = (
  node_id: string,
  status = 'succeeded',
  node_run_id = node_id
) => ({ type: 'node_finished' as const, node_id, node_run_id, status });

describe('Workflow canvas execution state', () => {
  test('untouched nodes have no mark; started, succeeded and failed have distinct states', () => {
    let state = emptyWorkflowRunState();
    expect(workflowNodeRunStatuses(state)).toEqual({});
    state = applyWorkflowRunEvent(state, start('start'));
    expect(workflowNodeRunStatuses(state)).toEqual({ start: 'running' });
    state = applyWorkflowRunEvent(state, finish('start'));
    state = applyWorkflowRunEvent(state, start('transform'));
    expect(workflowNodeRunStatuses(state)).toEqual({
      start: 'succeeded',
      transform: 'running'
    });
    state = applyWorkflowRunEvent(state, finish('transform', 'failed'));
    expect(workflowNodeRunStatuses(state)).toEqual({
      start: 'succeeded',
      transform: 'failed'
    });
    expect(workflowNodeRunStatuses(state).end).toBeUndefined();
  });

  test('parallel nodes stay independently active and unvisited branches stay blank', () => {
    let state = applyWorkflowRunEvent(emptyWorkflowRunState(), start('left'));
    state = applyWorkflowRunEvent(state, start('right'));
    state = applyWorkflowRunEvent(state, finish('left'));
    expect(workflowNodeRunStatuses(state)).toEqual({
      left: 'succeeded',
      right: 'running'
    });
  });

  test('finishing an older round does not clear a concurrent round spinner', () => {
    let state = applyWorkflowRunEvent(
      emptyWorkflowRunState(),
      start('body', 'round-1')
    );
    state = applyWorkflowRunEvent(state, start('body', 'round-2'));
    state = applyWorkflowRunEvent(
      state,
      finish('body', 'succeeded', 'round-1')
    );
    expect(workflowNodeRunStatuses(state).body).toBe('running');
    state = applyWorkflowRunEvent(
      state,
      finish('body', 'succeeded', 'round-2')
    );
    expect(workflowNodeRunStatuses(state).body).toBe('succeeded');
    state = applyWorkflowRunEvent(state, start('body', 'round-3'));
    expect(workflowNodeRunStatuses(state).body).toBe('running');
  });

  test('a flow failure stops outstanding spinners without inventing node success', () => {
    let state = applyWorkflowRunEvent(emptyWorkflowRunState(), start('node'));
    state = applyWorkflowRunEvent(state, {
      type: 'flow_failed',
      run_id: 'run-1',
      error: 'Execution failed'
    });
    expect(isWorkflowRunActive(state)).toBe(false);
    expect(workflowNodeRunStatuses(state).node).toBeUndefined();
    expect(state.failure).toBe('Execution failed');
  });

  test('stream gaps do not turn a running workflow into success or failure', () => {
    const state = applyWorkflowRunEvent(
      { ...emptyWorkflowRunState(), status: 'running' },
      { type: 'replay_gap', run_id: 'run-1' }
    );
    expect(state.status).toBe('running');
    expect(isWorkflowRunActive(state)).toBe(true);
  });

  test('waiting remains active; only the backend terminal event unlocks the canvas', () => {
    let state = applyWorkflowRunEvent(emptyWorkflowRunState(), {
      type: 'waiting_callback',
      run_id: 'run-1',
      status: 'waiting_callback'
    });
    expect(isWorkflowRunActive(state)).toBe(true);
    state = applyWorkflowRunEvent(state, {
      type: 'flow_finished',
      run_id: 'run-1',
      status: 'succeeded',
      output: { total: 3 }
    });
    expect(isWorkflowRunActive(state)).toBe(false);
    expect(state.output).toEqual({ total: 3 });
  });
});
