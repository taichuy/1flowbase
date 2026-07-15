import { describe, expect, test, vi } from 'vitest';

import type {
  JsBlockRunRequest,
  JsBlockWorkerLike
} from '@1flowbase/page-runtime';

import {
  createRestrictedBlockRuntimeHost,
  type RestrictedBlockRuntimeHostSnapshot
} from '../../lib/restricted-block-runtime-host';
import type { RestrictedBlockRunPlan } from '../../lib/restricted-block-loader';

const validSource = `
import { defineBlock } from '@1flowbase/block-sdk';
import { Text } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  render() {
    return Text({ children: 'Ready' });
  }
});
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
    requestId: 'restricted-block:block-1:code-1',
    blockId: 'block-1',
    source: validSource,
    props: { title: 'Hello' },
    state: { selected: false },
    contextSnapshot: { pageId: 'page-1' },
    limits: { timeoutMs: 1000, maxRenderDepth: 8, maxRenderNodes: 250 },
    ...overrides
  };
}

function createRunPlan(
  overrides: Partial<RestrictedBlockRunPlan> = {}
): RestrictedBlockRunPlan {
  return {
    ok: true,
    request: createRunRequest(),
    schemaValidationOptions: {
      maxDepth: 8,
      maxNodes: 250,
      allowedDataPermissions: ['query'],
      allowedActions: ['record.save'],
      allowedEvents: ['record.saved']
    },
    mediatorPolicy: {
      allowedEvents: ['record.saved'],
      allowedActions: ['record.save'],
      allowedQueries: ['records'],
      allowedDataOperations: ['query'],
      maxEventChainDepth: 4
    },
    ...overrides
  };
}

function createSubject(
  options: {
    runPlan?: RestrictedBlockRunPlan;
    handlers?: Parameters<typeof createRestrictedBlockRuntimeHost>[0]['handlers'];
  } = {}
): {
  worker: FakeWorker;
  host: ReturnType<typeof createRestrictedBlockRuntimeHost>;
} {
  const worker = new FakeWorker();
  const host = createRestrictedBlockRuntimeHost({
    runPlan: options.runPlan ?? createRunPlan(),
    workerFactory: () => worker,
    handlers: options.handlers
  });

  return { worker, host };
}

function expectFailedSnapshot(
  snapshot: RestrictedBlockRuntimeHostSnapshot
): asserts snapshot is RestrictedBlockRuntimeHostSnapshot & { status: 'failed' } {
  expect(snapshot.status).toBe('failed');
}

describe('restricted block runtime host controller', () => {
  test('creates a worker host from the run plan and sends the run request', () => {
    const runPlan = createRunPlan();
    const { worker, host } = createSubject({ runPlan });

    const snapshot = host.run();

    expect(worker.messages).toEqual([
      {
        direction: 'host_to_worker',
        type: 'run',
        request: runPlan.request
      }
    ]);
    expect(snapshot).toMatchObject({
      status: 'running',
      requestId: runPlan.request.requestId,
      blockId: runPlan.request.blockId,
      schemaValidationOptions: runPlan.schemaValidationOptions,
      logs: [],
      effects: [],
      rejections: []
    });
  });

  test('exposes a ready snapshot with schema, validation options, logs, effects, rejections, and mediator state', () => {
    const { worker, host } = createSubject();

    host.run();
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'log',
      requestId: 'restricted-block:block-1:code-1',
      level: 'info',
      message: 'rendering'
    });
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'event',
      requestId: 'restricted-block:block-1:code-1',
      name: 'record.saved',
      payload: { id: 'record-1' }
    });
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'rendered',
      requestId: 'restricted-block:block-1:code-1',
      schema: { primitive: 'Text', props: { children: 'Ready' } }
    });

    expect(host.getSnapshot()).toEqual({
      status: 'ready',
      requestId: 'restricted-block:block-1:code-1',
      blockId: 'block-1',
      schema: { primitive: 'Text', props: { children: 'Ready' } },
      schemaValidationOptions: {
        maxDepth: 8,
        maxNodes: 250,
        allowedDataPermissions: ['query'],
        allowedActions: ['record.save'],
        allowedEvents: ['record.saved']
      },
      logs: [
        {
          requestId: 'restricted-block:block-1:code-1',
          level: 'info',
          message: 'rendering'
        }
      ],
      effects: [
        {
          type: 'event',
          requestId: 'restricted-block:block-1:code-1',
          name: 'record.saved',
          payload: { id: 'record-1' }
        }
      ],
      rejections: [],
      mediatorState: {
        eventChains: {
          'restricted-block:block-1:code-1::restricted-block:block-1:code-1': 1
        }
      }
    });
    expect(worker.messages).toHaveLength(1);
  });

  test('reports source policy failure and worker errors as failed snapshots with stable run errors', () => {
    const blockedPlan = createRunPlan({
      request: createRunRequest({ source: 'window.location.href = "/bad";' })
    });
    const blocked = createSubject({ runPlan: blockedPlan });

    blocked.host.run();

    const sourceFailure = blocked.host.getSnapshot();
    expectFailedSnapshot(sourceFailure);
    expect(sourceFailure.error).toMatchObject({
      kind: 'source_policy_failed',
      message: 'JS block source policy validation failed.'
    });
    expect(blocked.worker.messages).toEqual([]);

    const failed = createSubject();
    failed.host.run();
    failed.worker.emitError('worker exploded');

    const workerFailure = failed.host.getSnapshot();
    expectFailedSnapshot(workerFailure);
    expect(workerFailure.error).toEqual({
      kind: 'runtime_error',
      message: 'worker exploded',
      errors: [
        {
          code: 'runtime_error',
          path: 'runtime',
          message: 'worker exploded'
        }
      ]
    });
  });

  test('resolves allowed action and data effects through mediator policy and injected handlers', () => {
    const dataHandler = vi.fn(() => ({ rows: [{ id: 'record-1' }] }));
    const actionHandler = vi.fn(() => ({ saved: true }));
    const { worker, host } = createSubject({
      handlers: {
        data: dataHandler,
        action: actionHandler
      }
    });

    host.run();
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'data',
      requestId: 'restricted-block:block-1:code-1',
      effectId: 'effect-data',
      queryId: 'records',
      params: { where: { id: 'record-1' } }
    });
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'action',
      requestId: 'restricted-block:block-1:code-1',
      effectId: 'effect-action',
      actionId: 'record.save',
      payload: { id: 'record-1' }
    });

    expect(dataHandler).toHaveBeenCalledWith({
      type: 'data',
      requestId: 'restricted-block:block-1:code-1',
      effectId: 'effect-data',
      queryId: 'records',
      params: { where: { id: 'record-1' } }
    });
    expect(actionHandler).toHaveBeenCalledWith({
      type: 'action',
      requestId: 'restricted-block:block-1:code-1',
      effectId: 'effect-action',
      actionId: 'record.save',
      payload: { id: 'record-1' }
    });
    expect(worker.messages).toEqual([
      {
        direction: 'host_to_worker',
        type: 'run',
        request: createRunRequest()
      },
      {
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: 'restricted-block:block-1:code-1',
        effectId: 'effect-data',
        ok: true,
        value: { rows: [{ id: 'record-1' }] }
      },
      {
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: 'restricted-block:block-1:code-1',
        effectId: 'effect-action',
        ok: true,
        value: { saved: true }
      }
    ]);
    expect(host.getSnapshot().effects).toEqual([
      {
        type: 'data',
        requestId: 'restricted-block:block-1:code-1',
        effectId: 'effect-data',
        queryId: 'records',
        params: { where: { id: 'record-1' } }
      },
      {
        type: 'action',
        requestId: 'restricted-block:block-1:code-1',
        effectId: 'effect-action',
        actionId: 'record.save',
        payload: { id: 'record-1' }
      }
    ]);
  });

  test('returns failed effect_result for denied effects', () => {
    const dataHandler = vi.fn();
    const actionHandler = vi.fn();
    const { worker, host } = createSubject({
      handlers: {
        data: dataHandler,
        action: actionHandler
      }
    });

    host.run();
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'data',
      requestId: 'restricted-block:block-1:code-1',
      effectId: 'effect-data',
      queryId: 'private_records'
    });
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'action',
      requestId: 'restricted-block:block-1:code-1',
      effectId: 'effect-action',
      actionId: 'record.delete'
    });

    expect(dataHandler).not.toHaveBeenCalled();
    expect(actionHandler).not.toHaveBeenCalled();
    expect(worker.messages.slice(1)).toEqual([
      {
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: 'restricted-block:block-1:code-1',
        effectId: 'effect-data',
        ok: false,
        error: {
          kind: 'runtime_error',
          message: 'Query is not allowed: private_records.',
          errors: [
            {
              code: 'query_denied',
              path: 'data.queryId',
              message: 'Query is not allowed: private_records.'
            }
          ]
        }
      },
      {
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: 'restricted-block:block-1:code-1',
        effectId: 'effect-action',
        ok: false,
        error: {
          kind: 'runtime_error',
          message: 'Action is not allowed: record.delete.',
          errors: [
            {
              code: 'action_denied',
              path: 'action.actionId',
              message: 'Action is not allowed: record.delete.'
            }
          ]
        }
      }
    ]);
  });

  test('disposes the current request and ignores late worker messages', () => {
    const { worker, host } = createSubject();

    host.run();
    host.dispose();
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'rendered',
      requestId: 'restricted-block:block-1:code-1',
      schema: { primitive: 'Text', props: { children: 'Late' } }
    });

    const snapshot = host.getSnapshot();
    expect(snapshot.status).toBe('disposed');
    expect(snapshot.schema).toBeUndefined();
    expect(snapshot.error).toBeUndefined();
    expect(worker.terminateCount).toBe(1);
  });

  test('returns snapshots and host state without exposing mutable runtime references', () => {
    const { worker, host } = createSubject();

    host.run();
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'log',
      requestId: 'restricted-block:block-1:code-1',
      level: 'info',
      message: 'rendering',
      data: { phase: 'start' }
    });
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'event',
      requestId: 'restricted-block:block-1:code-1',
      name: 'record.saved',
      payload: { id: 'record-1' }
    });
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'rendered',
      requestId: 'restricted-block:block-1:code-1',
      schema: {
        primitive: 'Stack',
        children: [{ primitive: 'Text', props: { children: 'Ready' } }]
      }
    });

    const snapshot = host.getSnapshot();
    const schema = snapshot.schema as {
      children: Array<{ props: { children: string } }>;
    };
    schema.children[0].props.children = 'Mutated';
    snapshot.logs[0].message = 'mutated log';
    (snapshot.logs[0].data as { phase: string }).phase = 'mutated';
    if (snapshot.effects[0].type === 'event') {
      snapshot.effects[0].payload = { id: 'mutated-record' };
    }
    snapshot.rejections.push({
      code: 'invalid_message',
      path: 'test',
      message: 'mutated rejection'
    });
    (
      snapshot.schemaValidationOptions.allowedActions as string[] | undefined
    )?.push('record.delete');
    snapshot.mediatorState!.eventChains.mutated = 99;

    const hostState = host.getHostState();
    hostState.requests['restricted-block:block-1:code-1']!.status = 'failed';

    expect(host.getSnapshot()).toEqual({
      status: 'ready',
      requestId: 'restricted-block:block-1:code-1',
      blockId: 'block-1',
      schema: {
        primitive: 'Stack',
        children: [{ primitive: 'Text', props: { children: 'Ready' } }]
      },
      schemaValidationOptions: {
        maxDepth: 8,
        maxNodes: 250,
        allowedDataPermissions: ['query'],
        allowedActions: ['record.save'],
        allowedEvents: ['record.saved']
      },
      logs: [
        {
          requestId: 'restricted-block:block-1:code-1',
          level: 'info',
          message: 'rendering',
          data: { phase: 'start' }
        }
      ],
      effects: [
        {
          type: 'event',
          requestId: 'restricted-block:block-1:code-1',
          name: 'record.saved',
          payload: { id: 'record-1' }
        }
      ],
      rejections: [],
      mediatorState: {
        eventChains: {
          'restricted-block:block-1:code-1::restricted-block:block-1:code-1': 1
        }
      }
    });
  });
});
