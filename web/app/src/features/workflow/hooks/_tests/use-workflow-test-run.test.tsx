import { act, cleanup, render } from '@testing-library/react';
import { useLayoutEffect, useRef } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiClientError } from '@1flowbase/api-client';
import {
  createDefaultWorkflowDocument,
  type FlowAuthoringDocument
} from '@1flowbase/flow-schema';

import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import * as runtime from '../../../agent-flow/api/runtime';
import { authenticateEditor } from '../../../agent-flow/_tests/fixtures/editor-auth';
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
function mount(
  applicationId: string,
  document: FlowAuthoringDocument,
  onStarted?: () => void
) {
  const commits: Commit[] = [];
  let current!: Run;
  function Consumer(props: {
    applicationId: string;
    document: FlowAuthoringDocument;
    onStarted?: () => void;
  }) {
    const run = useWorkflowTestRun(props.applicationId, props.document, props.onStarted);
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
    <Consumer applicationId={applicationId} document={document} onStarted={onStarted} />
  );
  return {
    commits,
    get current() {
      return current;
    },
    rerender(
      nextApplicationId: string,
      nextDocument: FlowAuthoringDocument,
      nextOnStarted = onStarted
    ) {
      view.rerender(
        <Consumer applicationId={nextApplicationId} document={nextDocument} onStarted={nextOnStarted} />
      );
    },
    unmount: () => view.unmount()
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

describe('useWorkflowTestRun admitted-start ownership', () => {
  it('notifies synchronously before transport once, guarding repeated and reentrant starts', async () => {
    const held = deferred<void>(undefined);
    const order: string[] = [];
    const transport = vi.spyOn(runtime, 'startFlowDebugRunStream').mockImplementation(
      async (_applicationId, _input, _csrfToken, handlers) => {
        order.push('transport');
        handlers.getAbortController?.(new AbortController());
        await held.promise;
      }
    );
    const onStarted = vi.fn(() => {
      order.push('started');
      expect(owner.current.state.status).toBe('idle');
      track(owner.current.start(inputs));
    });
    const owner: ReturnType<typeof mount> = mount('app-A', documentFor('app-A'), onStarted);

    await act(async () => { track(owner.current.start(inputs)); });

    expect(order).toEqual(['started', 'transport']);
    expect(onStarted).toHaveBeenCalledTimes(1);
    expect(owner.current.state.status).toBe('starting');
    await act(async () => { await owner.current.start(inputs); });
    expect(onStarted).toHaveBeenCalledTimes(1);
    expect(transport).toHaveBeenCalledTimes(1);
  });

  it.each(['permission', 'csrf', 'missing-start-node'] as const)(
    'does not notify or submit when the actual %s guard rejects the attempt',
    async (reason) => {
      if (reason === 'permission') authenticateEditor([], 'member');
      if (reason === 'csrf') useAuthStore.setState({ csrfToken: null });
      const document = documentFor('app-A');
      if (reason === 'missing-start-node') {
        document.graph.nodes = document.graph.nodes.filter((node) => node.type !== 'workflow_start');
      }
      const onStarted = vi.fn();
      const transport = vi.spyOn(runtime, 'startFlowDebugRunStream').mockResolvedValue(undefined);
      const owner = mount('app-A', document, onStarted);
      expect(owner.current.canRun).toBe(reason === 'missing-start-node');

      await act(async () => { await owner.current.start(inputs); });

      expect(onStarted).not.toHaveBeenCalled();
      expect(transport).not.toHaveBeenCalled();
      expect(owner.current.state.status).toBe(reason === 'missing-start-node' ? 'failed' : 'idle');
    }
  );

  it('uses the latest same-application callback without restarting an active observation', async () => {
    const { start, streams } = heldStreams();
    const document = documentFor('app-A');
    const firstCallback = vi.fn();
    const currentCallback = vi.fn();
    const followingCallback = vi.fn();
    const owner = mount('app-A', document, firstCallback);
    owner.rerender('app-A', document, currentCallback);
    await act(async () => { track(owner.current.start(inputs)); });
    const firstStream = streams.get('app-A')!;
    expect(firstCallback).not.toHaveBeenCalled();
    expect(currentCallback).toHaveBeenCalledTimes(1);

    owner.rerender('app-A', { ...document }, followingCallback);
    await act(async () => { await owner.current.start(inputs); });
    expect(followingCallback).not.toHaveBeenCalled();
    expect(firstStream.controller.signal.aborted).toBe(false);
    expect(start).toHaveBeenCalledTimes(1);
    await act(async () => {
      firstStream.handlers.onEvent({ type: 'flow_finished', run_id: 'run-A1', status: 'succeeded', output: {} });
      firstStream.release();
    });
    expect(currentCallback).toHaveBeenCalledTimes(1);
    expect(followingCallback).not.toHaveBeenCalled();

    await act(async () => { track(owner.current.start(inputs)); });
    expect(followingCallback).toHaveBeenCalledTimes(1);
    expect(start).toHaveBeenCalledTimes(2);
  });

  it('rejects retained starts after another application commits, after A returns and after unmount', async () => {
    const { start, streams } = heldStreams();
    const firstA = vi.fn();
    const currentB = vi.fn();
    const nextA = vi.fn();
    const owner = mount('app-A', documentFor('app-A'), firstA);
    const retainedFirstA = owner.current.start;
    owner.rerender('app-B', documentFor('app-B'), currentB);

    await act(async () => { track(retainedFirstA(inputs)); });

    expect(firstA).not.toHaveBeenCalled();
    expect(currentB).not.toHaveBeenCalled();
    expect(start).not.toHaveBeenCalled();
    await act(async () => { track(owner.current.start(inputs)); });
    const streamB = streams.get('app-B')!;
    expect(currentB).toHaveBeenCalledTimes(1);
    await act(async () => {
      streamB.handlers.onEvent({ type: 'flow_finished', run_id: 'run-B', status: 'succeeded', output: {} });
      streamB.release();
    });
    owner.rerender('app-A', documentFor('app-A'), nextA);
    await act(async () => { track(retainedFirstA(inputs)); });
    expect(firstA).not.toHaveBeenCalled();
    expect(nextA).not.toHaveBeenCalled();
    expect(start).toHaveBeenCalledTimes(1);
    await act(async () => { track(owner.current.start(inputs)); });
    const streamA = streams.get('app-A')!;
    expect(nextA).toHaveBeenCalledTimes(1);
    expect(start.mock.calls.map(([id]) => id)).toEqual(['app-B', 'app-A']);
    await act(async () => {
      streamA.handlers.onEvent({ type: 'flow_finished', run_id: 'run-A2', status: 'succeeded', output: {} });
      streamA.release();
    });
    const retainedAfterUnmount = owner.current.start;
    owner.unmount();
    await act(async () => { track(retainedAfterUnmount(inputs)); });
    expect(nextA).toHaveBeenCalledTimes(1);
    expect(currentB).toHaveBeenCalledTimes(1);
    expect(start).toHaveBeenCalledTimes(2);
  });

  it('notifies once for an admitted attempt even when the transport immediately rejects it', async () => {
    const onStarted = vi.fn();
    const transport = vi.spyOn(runtime, 'startFlowDebugRunStream').mockRejectedValue(
      new ApiClientError({ status: 403, message: 'Server rejected the admitted attempt' })
    );
    const owner = mount('app-A', documentFor('app-A'), onStarted);

    await act(async () => { await track(owner.current.start(inputs)); });

    expect(onStarted).toHaveBeenCalledTimes(1);
    expect(transport).toHaveBeenCalledTimes(1);
    expect(owner.current.state).toMatchObject({
      status: 'failed', failure: 'Server rejected the admitted attempt'
    });
  });
});
