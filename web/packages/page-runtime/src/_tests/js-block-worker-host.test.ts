import { describe, expect, test } from 'vitest';

import {
  createJsBlockWorkerHost,
  type JsBlockRunRequest,
  type JsBlockWorkerLike
} from '../index';

const validSource = `
import { Text } from '@1flowbase/block-renderer/antd-facade';

async function main() {
  return { view: Text({ children: 'Ready' }), outputs: {} };
}
export default { main };
`;

class FakeWorker implements JsBlockWorkerLike {
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onerror: ((event: { message?: string }) => void) | null = null;
  onmessageerror: ((event: { message?: string }) => void) | null = null;
  readonly messages: unknown[] = [];
  terminateCount = 0;

  postMessage(message: unknown): void {
    this.messages.push(message);
  }

  terminate(): void {
    this.terminateCount += 1;
  }

  emitMessage(data: unknown): void {
    this.onmessage?.({ data });
  }

  emitError(message = 'worker failed'): void {
    this.onerror?.({ message });
  }
}

function createRunRequest(
  overrides: Partial<JsBlockRunRequest> = {}
): JsBlockRunRequest {
  return {
    requestId: 'request-1',
    blockId: 'block-1',
    source: validSource,
    props: {},
    state: {},
    contextSnapshot: { pageId: 'page-1' },
    limits: { timeoutMs: 1000, maxRenderDepth: 8, maxRenderNodes: 250 },
    ...overrides
  };
}

function createManualTimers() {
  const callbacks = new Map<number, () => void>();
  let nextHandle = 1;

  return {
    schedule(callback: () => void): number {
      const handle = nextHandle;
      nextHandle += 1;
      callbacks.set(handle, callback);
      return handle;
    },
    clear(handle: number): void {
      callbacks.delete(handle);
    },
    fire(handle: number): void {
      callbacks.get(handle)?.();
    },
    get size(): number {
      return callbacks.size;
    }
  };
}

describe('JS block worker host adapter', () => {
  test('waits for worker ready before sending run and starting the user budget', () => {
    const worker = new FakeWorker();
    const host = createJsBlockWorkerHost({
      workerFactory: () => worker
    });

    host.init();
    host.run(createRunRequest());
    expect(worker.messages).toEqual([
      { direction: 'host_to_worker', type: 'init' }
    ]);

    worker.emitMessage({ direction: 'worker_to_host', type: 'ready' });
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'completed',
      requestId: 'request-1',
      view: { primitive: 'Text', props: { children: 'Ready' } },
      outputs: {}
    });

    expect(worker.messages).toEqual([
      { direction: 'host_to_worker', type: 'init' },
      {
        direction: 'host_to_worker',
        type: 'run',
        request: createRunRequest()
      }
    ]);
    expect(host.getState().requests['request-1']).toMatchObject({
      status: 'ready',
      result: { ok: true, requestId: 'request-1' }
    });
  });

  test('does not send a worker run when source policy fails', () => {
    const worker = new FakeWorker();
    const host = createJsBlockWorkerHost({
      workerFactory: () => worker
    });

    host.run(createRunRequest({ source: 'window.location.href;' }));

    expect(worker.messages).toEqual([]);
    expect(host.getState().requests['request-1']).toMatchObject({
      status: 'failed',
      result: {
        ok: false,
        error: { kind: 'source_policy_failed' }
      }
    });
  });

  test('times out pending requests, clears timers, and terminates the worker once', () => {
    const worker = new FakeWorker();
    const timers = createManualTimers();
    const host = createJsBlockWorkerHost({
      workerFactory: () => worker,
      scheduleTimeout: (callback) => timers.schedule(callback),
      clearScheduledTimeout: (handle) => timers.clear(handle as number)
    });

    host.run(createRunRequest());
    expect(timers.size).toBe(1);
    expect(worker.messages).toEqual([
      { direction: 'host_to_worker', type: 'init' }
    ]);

    worker.emitMessage({ direction: 'worker_to_host', type: 'ready' });
    expect(timers.size).toBe(1);

    timers.fire(2);
    timers.fire(2);

    expect(host.getState().requests['request-1']).toMatchObject({
      status: 'timed_out',
      result: {
        ok: false,
        error: { kind: 'runtime_timeout' }
      }
    });
    expect(timers.size).toBe(0);
    expect(worker.terminateCount).toBe(1);
  });

  test('reports startup timeout separately without spending the user runtime budget', () => {
    const worker = new FakeWorker();
    const timers = createManualTimers();
    const host = createJsBlockWorkerHost({
      workerFactory: () => worker,
      startupTimeoutMs: 3000,
      scheduleTimeout: (callback) => timers.schedule(callback),
      clearScheduledTimeout: (handle) => timers.clear(handle as number)
    });

    host.run(createRunRequest());
    timers.fire(1);

    expect(host.getState().requests['request-1']).toMatchObject({
      status: 'failed',
      phase: 'failed',
      result: {
        ok: false,
        error: { kind: 'worker_startup_timeout' }
      }
    });
    expect(worker.messages).toEqual([
      { direction: 'host_to_worker', type: 'init' }
    ]);
    expect(worker.terminateCount).toBe(1);
  });

  test('maps startup worker errors into worker_crash and clears startup timing', () => {
    const worker = new FakeWorker();
    const timers = createManualTimers();
    const host = createJsBlockWorkerHost({
      workerFactory: () => worker,
      scheduleTimeout: (callback) => timers.schedule(callback),
      clearScheduledTimeout: (handle) => timers.clear(handle as number)
    });

    host.run(createRunRequest());
    worker.emitError('boom');

    expect(host.getState().requests['request-1']).toMatchObject({
      status: 'failed',
      result: {
        ok: false,
        error: { kind: 'worker_crash' }
      }
    });
    expect(timers.size).toBe(0);
  });

  test('forwards host effect results to the worker while a request is pending', () => {
    const worker = new FakeWorker();
    const host = createJsBlockWorkerHost({
      workerFactory: () => worker
    });

    host.run(createRunRequest());
    worker.emitMessage({ direction: 'worker_to_host', type: 'ready' });
    host.resolveEffect({
      direction: 'host_to_worker',
      type: 'effect_result',
      requestId: 'request-1',
      effectId: 'request-1:effect-1',
      ok: true,
      value: { title: 'Ready' }
    });

    expect(worker.messages).toEqual([
      { direction: 'host_to_worker', type: 'init' },
      {
        direction: 'host_to_worker',
        type: 'run',
        request: createRunRequest()
      },
      {
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: 'request-1',
        effectId: 'request-1:effect-1',
        ok: true,
        value: { title: 'Ready' }
      }
    ]);
    expect(host.getState().requests['request-1']?.status).toBe('pending');
  });

  test('bridges worker effects through mediator policy and host effect resolution', () => {
    const worker = new FakeWorker();
    const host = createJsBlockWorkerHost({
      workerFactory: () => worker,
      effectBridge: {
        policy: {
          allowedEvents: ['record.saved']
        },
        getContext: () => ({ tickId: 'tick-1' }),
        handlers: {
          interface: () => ({ id: 'record-1', title: 'Ready' })
        }
      }
    });

    host.run(createRunRequest());
    worker.emitMessage({ direction: 'worker_to_host', type: 'ready' });
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'interface',
      requestId: 'request-1',
      effectId: 'effect-data',
      method: 'GET',
      path: '/api/console/test',
      request: { query: { id: 'record-1' } }
    });
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'event',
      requestId: 'request-1',
      name: 'record.saved',
      payload: { id: 'record-1' }
    });

    expect(worker.messages).toEqual([
      { direction: 'host_to_worker', type: 'init' },
      {
        direction: 'host_to_worker',
        type: 'run',
        request: createRunRequest()
      },
      {
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: 'request-1',
        effectId: 'effect-data',
        ok: true,
        value: { id: 'record-1', title: 'Ready' }
      }
    ]);
    expect(host.getEffectMediatorState()).toEqual({
      eventChains: {
        'request-1::tick-1': 1
      }
    });
  });

  test('dispose cleans up handlers, timers, and ignores late worker messages', () => {
    const worker = new FakeWorker();
    const timers = createManualTimers();
    const host = createJsBlockWorkerHost({
      workerFactory: () => worker,
      scheduleTimeout: (callback) => timers.schedule(callback),
      clearScheduledTimeout: (handle) => timers.clear(handle as number)
    });

    host.run(createRunRequest());
    host.dispose('request-1');
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'completed',
      requestId: 'request-1',
      view: { primitive: 'Text' },
      outputs: {}
    });

    expect(host.getState().requests['request-1']).toMatchObject({
      status: 'disposed'
    });
    expect(timers.size).toBe(0);
    expect(worker.terminateCount).toBe(1);

    host.dispose();
    expect(worker.terminateCount).toBe(1);
  });
});
