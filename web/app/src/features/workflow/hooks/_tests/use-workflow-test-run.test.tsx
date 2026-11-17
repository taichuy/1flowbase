import { act, cleanup, render } from '@testing-library/react';
import { useLayoutEffect, useRef } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  createDefaultWorkflowDocument,
  type FlowAuthoringDocument
} from '@1flowbase/flow-schema';

import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import * as runtime from '../../../agent-flow/api/runtime';
import { useWorkflowTestRun } from '../use-workflow-test-run';

type Run = ReturnType<typeof useWorkflowTestRun>;
interface Commit {
  applicationId: string;
  instance: object;
  state: Run['state'];
  active: boolean;
  nodeStatuses: Run['nodeStatuses'];
}
const releasePending: Array<() => void> = [];
const tasks: Promise<void>[] = [];
function deferred<T>(cleanupValue: T) {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  releasePending.push(() => resolve(cleanupValue));
  return { promise, resolve };
}
function track(task: Promise<void>) {
  tasks.push(task);
  void task.catch(() => {});
  return task;
}
function documentFor(applicationId: string) {
  const document = createDefaultWorkflowDocument({ flowId: applicationId });
  document.graph.nodes.find(
    (node) => node.type === 'workflow_start'
  )!.config.input_fields = [
    {
      key: 'customer_id',
      label: 'Customer ID',
      inputType: 'text',
      valueType: 'string',
      required: true
    },
    {
      key: 'force',
      label: 'Force',
      inputType: 'checkbox',
      valueType: 'boolean',
      required: false,
      defaultValue: false
    }
  ];
  return document;
}
const inputs = { customer_id: 'C-42' };
function detail(
  applicationId: string,
  runId: string,
  status: string,
  output: Record<string, unknown> = {}
): runtime.FlowDebugRunDetail {
  return {
    flow_run: {
      id: runId,
      application_id: applicationId,
      flow_id: applicationId,
      draft_id: 'draft-1',
      compiled_plan_id: 'plan-1',
      run_mode: 'debug_flow_run',
      status,
      target_node_id: null,
      input_payload: {},
      output_payload: output,
      error_payload: null,
      created_by: 'user-1',
      started_at: '2026-11-17T00:00:00Z',
      finished_at: status === 'running' ? null : '2026-11-17T00:00:01Z',
      created_at: '2026-11-17T00:00:00Z'
    },
    node_runs: [],
    checkpoints: [],
    callback_tasks: [],
    events: []
  };
}
function mount(applicationId: string, document: FlowAuthoringDocument) {
  const commits: Commit[] = [];
  let current!: Run;
  function Consumer(props: {
    applicationId: string;
    document: FlowAuthoringDocument;
  }) {
    const run = useWorkflowTestRun(props.applicationId, props.document);
    const instance = useRef({});
    useLayoutEffect(() => {
      current = run;
      commits.push({
        applicationId: props.applicationId,
        instance: instance.current,
        state: structuredClone(run.state),
        active: run.active,
        nodeStatuses: { ...run.nodeStatuses }
      });
    });
    return <output aria-label="Workflow run status">{run.state.status}</output>;
  }
  const view = render(
    <Consumer applicationId={applicationId} document={document} />
  );
  return {
    commits,
    get current() {
      return current;
    },
    rerender(nextApplicationId: string, nextDocument: FlowAuthoringDocument) {
      view.rerender(
        <Consumer applicationId={nextApplicationId} document={nextDocument} />
      );
    }
  };
}
function heldStreams() {
  const streams = new Map<
    string,
    {
      handlers: runtime.FlowDebugRunStreamHandlers;
      controller: AbortController;
      release: () => void;
    }
  >();
  const start = vi.spyOn(runtime, 'startFlowDebugRunStream').mockImplementation(
    async (applicationId, _input, _csrfToken, handlers) => {
      const held = deferred<void>(undefined);
      const controller = new AbortController();
      handlers.getAbortController?.(controller);
      streams.set(applicationId, {
        handlers,
        controller,
        release: () => held.resolve(undefined)
      });
      await held.promise;
    }
  );
  return { start, streams };
}
function startEvents(handlers: runtime.FlowDebugRunStreamHandlers, suffix: string) {
  handlers.onEvent({
    type: 'flow_started',
    run_id: `run-${suffix}`,
    status: 'running'
  });
  handlers.onEvent({
    type: 'node_started',
    run_id: `run-${suffix}`,
    node_run_id: `node-run-${suffix}`,
    node_id: 'node-workflow-start',
    node_type: 'workflow_start',
    title: `Start ${suffix}`
  });
}
beforeEach(() => {
  vi.useFakeTimers();
  resetAuthStore();
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-token',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: null
  });
});
afterEach(async () => {
  cleanup();
  await act(async () => {
    releasePending.splice(0).forEach((release) => release());
    await Promise.allSettled(tasks.splice(0));
  });
  vi.useRealTimers();
  vi.restoreAllMocks();
  resetAuthStore();
});

describe('useWorkflowTestRun application provenance', () => {
  it('exposes idle state without A output in the first committed B render of the same instance', async () => {
    const start = vi.spyOn(runtime, 'startFlowDebugRunStream').mockImplementation(
      async (_applicationId, _input, _csrfToken, handlers) => {
        handlers.onEvent({ type: 'flow_started', run_id: 'run-A', status: 'running' });
        handlers.onEvent({
          type: 'flow_finished',
          run_id: 'run-A',
          status: 'succeeded',
          output: { answer: 'A output' }
        });
      }
    );
    const owner = mount('app-A', documentFor('app-A'));
    expect(owner.current.canRun).toBe(true);
    await act(async () => {
      await track(owner.current.start(inputs));
    });
    expect(owner.current.state).toMatchObject({
      runId: 'run-A', status: 'succeeded', output: { answer: 'A output' }
    });
    const instanceA = owner.commits.at(-1)!.instance;

    owner.rerender('app-B', documentFor('app-B'));

    // Layout effects observe the committed consumer value before passive resets run.
    const firstB = owner.commits.find((commit) => commit.applicationId === 'app-B')!;
    expect(firstB.instance).toBe(instanceA);
    expect(firstB).toMatchObject({
      active: false,
      nodeStatuses: {},
      state: {
        runId: null,
        status: 'idle',
        executions: {},
        output: null,
        failure: null,
        connectionLost: false
      }
    });
    expect(start).toHaveBeenCalledTimes(1);
  });

  it('keeps B active when an A presentation timer and late stream events settle', async () => {
    const { start, streams } = heldStreams();
    const snapshot = vi.spyOn(runtime, 'fetchApplicationRunDebugSnapshot');
    const owner = mount('app-A', documentFor('app-A'));
    await act(async () => {
      track(owner.current.start(inputs));
    });
    const streamA = streams.get('app-A')!;
    await act(async () => startEvents(streamA.handlers, 'A'));
    expect(owner.current.nodeStatuses).toEqual({ 'node-workflow-start': 'running' });
    await act(async () => {
      streamA.handlers.onEvent({
        type: 'node_finished',
        run_id: 'run-A',
        node_run_id: 'node-run-A',
        node_id: 'node-workflow-start',
        status: 'succeeded'
      });
    });
    expect(vi.getTimerCount()).toBe(1);

    owner.rerender('app-B', documentFor('app-B'));
    expect(streamA.controller.signal.aborted).toBe(true);
    await act(async () => {
      track(owner.current.start(inputs));
    });
    const streamB = streams.get('app-B')!;
    await act(async () => startEvents(streamB.handlers, 'B'));
    await act(async () => {
      streamA.handlers.onEvent({
        type: 'flow_finished', run_id: 'run-A', status: 'succeeded',
        output: { answer: 'late A output' }
      });
      streamA.release();
      await vi.advanceTimersByTimeAsync(5000);
    });

    expect(owner.current.state).toMatchObject({
      runId: 'run-B', status: 'running', output: null
    });
    expect(owner.current.active).toBe(true);
    expect(owner.current.nodeStatuses).toEqual({ 'node-workflow-start': 'running' });
    expect(streamB.controller.signal.aborted).toBe(false);
    expect(start.mock.calls.map(([id]) => id)).toEqual(['app-A', 'app-B']);
    expect(snapshot).not.toHaveBeenCalled();
  });

  it('preserves an active observation when only the same application document changes', async () => {
    const { start, streams } = heldStreams();
    const document = documentFor('app-A');
    const owner = mount('app-A', document);
    await act(async () => {
      track(owner.current.start(inputs));
    });
    const stream = streams.get('app-A')!;
    await act(async () => startEvents(stream.handlers, 'A'));
    const edited = structuredClone(document);
    edited.graph.nodes[0].alias = 'Edited start alias';

    owner.rerender('app-A', edited);

    expect(owner.current.state).toMatchObject({ runId: 'run-A', status: 'running' });
    expect(owner.current.active).toBe(true);
    expect(owner.current.nodeStatuses).toEqual({ 'node-workflow-start': 'running' });
    expect(stream.controller.signal.aborted).toBe(false);
    await act(async () => {
      await owner.current.start(inputs);
    });
    expect(start).toHaveBeenCalledExactlyOnceWith(
      'app-A',
      { input_payload: { 'node-workflow-start': { customer_id: 'C-42', force: false } }, document },
      'csrf-token',
      expect.any(Object)
    );
    await act(async () => {
      stream.handlers.onEvent({
        type: 'flow_finished', run_id: 'run-A', status: 'succeeded',
        output: { answer: 'same application result' }
      });
      stream.release();
    });
    expect(owner.current.state.output).toEqual({ answer: 'same application result' });
  });

  it('submits once and polls the accepted run serially until its terminal snapshot', async () => {
    const first = deferred(detail('app-A', 'run-A', 'succeeded'));
    const second = deferred(detail('app-A', 'run-A', 'succeeded'));
    const snapshot = vi.spyOn(runtime, 'fetchApplicationRunDebugSnapshot')
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const start = vi.spyOn(runtime, 'startFlowDebugRunStream').mockImplementation(
      async (_applicationId, _input, _csrfToken, handlers) => {
        handlers.onEvent({ type: 'flow_accepted', run_id: 'run-A', status: 'queued' });
        throw new Error('transport lost after acceptance');
      }
    );
    const document = documentFor('app-A');
    const owner = mount('app-A', document);
    let task!: Promise<void>;
    await act(async () => {
      task = track(owner.current.start(inputs));
    });
    expect(snapshot).toHaveBeenCalledExactlyOnceWith('app-A', 'run-A');
    await act(async () => {
      await owner.current.start(inputs);
      await vi.advanceTimersByTimeAsync(9000);
    });
    expect(snapshot).toHaveBeenCalledTimes(1);
    expect(start).toHaveBeenCalledExactlyOnceWith(
      'app-A',
      { input_payload: { 'node-workflow-start': { customer_id: 'C-42', force: false } }, document },
      'csrf-token',
      expect.any(Object)
    );
    await act(async () => first.resolve(detail('app-A', 'run-A', 'running')));
    await act(async () => vi.advanceTimersByTimeAsync(1499));
    expect(snapshot).toHaveBeenCalledTimes(1);
    await act(async () => vi.advanceTimersByTimeAsync(1));
    expect(snapshot.mock.calls).toEqual([['app-A', 'run-A'], ['app-A', 'run-A']]);
    await act(async () => {
      second.resolve(detail('app-A', 'run-A', 'succeeded', { answer: 'A snapshot' }));
      await task;
    });
    expect(owner.current.state).toMatchObject({
      runId: 'run-A', status: 'succeeded', output: { answer: 'A snapshot' },
      connectionLost: false
    });
    expect(owner.current.active).toBe(false);
    await act(async () => vi.advanceTimersByTimeAsync(6000));
    expect(snapshot).toHaveBeenCalledTimes(2);
    expect(start).toHaveBeenCalledTimes(1);
  });

  it('ignores an in-flight A snapshot after B starts in the same consumer', async () => {
    const oldSnapshot = deferred(detail('app-A', 'run-A', 'succeeded'));
    const snapshot = vi.spyOn(runtime, 'fetchApplicationRunDebugSnapshot')
      .mockReturnValueOnce(oldSnapshot.promise);
    const heldB = deferred<void>(undefined);
    let handlersB!: runtime.FlowDebugRunStreamHandlers;
    vi.spyOn(runtime, 'startFlowDebugRunStream').mockImplementation(
      async (applicationId, _input, _csrfToken, handlers) => {
        if (applicationId === 'app-A') {
          handlers.onEvent({ type: 'flow_accepted', run_id: 'run-A', status: 'queued' });
          return;
        }
        handlersB = handlers;
        await heldB.promise;
      }
    );
    const owner = mount('app-A', documentFor('app-A'));
    await act(async () => { track(owner.current.start(inputs)); });
    expect(snapshot).toHaveBeenCalledExactlyOnceWith('app-A', 'run-A');
    owner.rerender('app-B', documentFor('app-B'));
    await act(async () => { track(owner.current.start(inputs)); });
    await act(async () => startEvents(handlersB, 'B'));
    await act(async () => {
      oldSnapshot.resolve(detail('app-A', 'run-A', 'succeeded', { answer: 'stale A snapshot' }));
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(owner.current.state).toMatchObject({ runId: 'run-B', status: 'running', output: null });
    expect(owner.current.nodeStatuses).toEqual({ 'node-workflow-start': 'running' });
    expect(owner.current.active).toBe(true);
    expect(snapshot).toHaveBeenCalledTimes(1);
  });
});
