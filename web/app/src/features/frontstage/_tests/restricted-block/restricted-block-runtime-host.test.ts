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
    requestId: 'restricted-block:block-1:code-1',
    blockId: 'block-1',
    program: { kind: 'source', source: validSource },
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

      allowedEvents: ['record.saved']
    },
    mediatorPolicy: {
      allowedEvents: ['record.saved'],

      maxEventChainDepth: 4
    },
    ...overrides
  };
}

function createSubject(
  options: {
    runPlan?: RestrictedBlockRunPlan;
    handlers?: Parameters<
      typeof createRestrictedBlockRuntimeHost
    >[0]['handlers'];
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
): asserts snapshot is RestrictedBlockRuntimeHostSnapshot & {
  status: 'failed';
} {
  expect(snapshot.status).toBe('failed');
}

function emitWorkerReady(worker: FakeWorker): void {
  worker.emitMessage({ direction: 'worker_to_host', type: 'ready' });
}

describe('restricted block runtime host controller', () => {
  test('creates a worker host from the run plan and sends the run request', () => {
    const authoritativeSourceSha256 =
      'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
    const runPlan = createRunPlan({
      request: createRunRequest({
        program: {
          kind: 'source',
          source: validSource,
          sourceSha256: authoritativeSourceSha256
        }
      })
    });
    const { worker, host } = createSubject({ runPlan });

    host.run();

    expect(worker.messages).toEqual([
      { direction: 'host_to_worker', type: 'init' }
    ]);
    emitWorkerReady(worker);

    expect(worker.messages).toEqual([
      { direction: 'host_to_worker', type: 'init' },
      {
        direction: 'host_to_worker',
        type: 'run',
        request: expect.objectContaining({
          requestId: runPlan.request.requestId,
          program: expect.objectContaining({ kind: 'compiled_artifact' })
        })
      }
    ]);
    expect(host.getSnapshot()).toMatchObject({
      status: 'running',
      requestId: runPlan.request.requestId,
      blockId: runPlan.request.blockId,
      schemaValidationOptions: runPlan.schemaValidationOptions,
      compiledArtifact: {
        format: '1flowbase/js-block-compiled-artifact',
        sourceSha256: authoritativeSourceSha256
      },
      logs: [],
      effects: [],
      rejections: []
    });
  });

  test('exposes a ready snapshot with schema, validation options, logs, effects, rejections, and mediator state', () => {
    const { worker, host } = createSubject();

    host.run();
    emitWorkerReady(worker);
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
      type: 'completed',
      requestId: 'restricted-block:block-1:code-1',
      view: { primitive: 'Text', props: { children: 'Ready' } },
      outputs: {}
    });

    expect(host.getSnapshot()).toEqual({
      status: 'ready',
      phase: 'ready',
      requestId: 'restricted-block:block-1:code-1',
      blockId: 'block-1',
      view: { primitive: 'Text', props: { children: 'Ready' } },
      outputs: {},
      compiledArtifact: expect.objectContaining({
        format: '1flowbase/js-block-compiled-artifact',
        sourceSha256: expect.any(String)
      }),
      schemaValidationOptions: {
        maxDepth: 8,
        maxNodes: 250,
        allowedDataPermissions: ['query'],

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
      interfaceCalls: [],
      mediatorState: {
        eventChains: {
          'restricted-block:block-1:code-1::restricted-block:block-1:code-1': 1
        }
      }
    });
    expect(worker.messages).toHaveLength(2);
  });

  test('reports source policy failure and worker errors as failed snapshots with stable run errors', () => {
    const blockedPlan = createRunPlan({
      request: createRunRequest({
        program: {
          kind: 'source',
          source: 'window.location.href = "/bad";'
        }
      })
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
      kind: 'worker_crash',
      message: 'worker exploded',
      errors: [
        {
          code: 'runtime_error',
          path: 'worker',
          message: 'worker exploded'
        }
      ]
    });
  });

  test('resolves complete interface routes through injected handlers', () => {
    const interfaceHandler = vi.fn(() => ({ rows: [{ id: 'record-1' }] }));
    const { worker, host } = createSubject({
      handlers: { interface: interfaceHandler }
    });

    host.run();
    emitWorkerReady(worker);
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'interface',
      requestId: 'restricted-block:block-1:code-1',
      effectId: 'effect-interface',
      method: 'GET',
      path: '/api/console/test',
      request: { query: { id: 'record-1' } }
    });

    expect(interfaceHandler).toHaveBeenCalledWith({
      type: 'interface',
      requestId: 'restricted-block:block-1:code-1',
      effectId: 'effect-interface',
      method: 'GET',
      path: '/api/console/test',
      request: { query: { id: 'record-1' } }
    });
    expect(worker.messages).toEqual([
      { direction: 'host_to_worker', type: 'init' },
      {
        direction: 'host_to_worker',
        type: 'run',
        request: expect.objectContaining({
          requestId: 'restricted-block:block-1:code-1',
          program: expect.objectContaining({ kind: 'compiled_artifact' })
        })
      },
      {
        direction: 'host_to_worker',
        type: 'effect_result',
        requestId: 'restricted-block:block-1:code-1',
        effectId: 'effect-interface',
        ok: true,
        value: { rows: [{ id: 'record-1' }] }
      }
    ]);
    expect(host.getSnapshot().effects).toEqual([
      {
        type: 'interface',
        requestId: 'restricted-block:block-1:code-1',
        effectId: 'effect-interface',
        method: 'GET',
        path: '/api/console/test',
        request: { query: { id: 'record-1' } }
      }
    ]);
  });

  test('rejects malformed worker routes before invoking the host handler', () => {
    const interfaceHandler = vi.fn();
    const { worker, host } = createSubject({
      handlers: { interface: interfaceHandler }
    });

    host.run();
    emitWorkerReady(worker);
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'interface',
      requestId: 'restricted-block:block-1:code-1',
      effectId: 'effect-interface',
      method: 'GET'
    });

    expect(interfaceHandler).not.toHaveBeenCalled();
    expect(worker.messages.slice(2)).toEqual([]);
    expect(host.getSnapshot().rejections).toContainEqual(
      expect.objectContaining({
        code: 'invalid_message',
        path: 'message.path'
      })
    );
  });

  test('disposes the current request and ignores late worker messages', () => {
    const { worker, host } = createSubject();

    host.run();
    emitWorkerReady(worker);
    host.dispose();
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'completed',
      requestId: 'restricted-block:block-1:code-1',
      view: { primitive: 'Text', props: { children: 'Late' } },
      outputs: {}
    });

    const snapshot = host.getSnapshot();
    expect(snapshot.status).toBe('disposed');
    expect(snapshot.view).toBeUndefined();
    expect(snapshot.error).toBeUndefined();
    expect(worker.terminateCount).toBe(1);
  });

  test('returns snapshots and host state without exposing mutable runtime references', () => {
    const { worker, host } = createSubject();

    host.run();
    emitWorkerReady(worker);
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
      type: 'completed',
      requestId: 'restricted-block:block-1:code-1',
      view: {
        primitive: 'Stack',
        children: [{ primitive: 'Text', props: { children: 'Ready' } }]
      },
      outputs: {}
    });

    const snapshot = host.getSnapshot();
    const schema = snapshot.view as {
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
    snapshot.compiledArtifact!.program.executableBody = 'mutated artifact';

    const hostState = host.getHostState();
    hostState.requests['restricted-block:block-1:code-1']!.status = 'failed';

    expect(host.getSnapshot()).toEqual({
      status: 'ready',
      phase: 'ready',
      requestId: 'restricted-block:block-1:code-1',
      blockId: 'block-1',
      view: {
        primitive: 'Stack',
        children: [{ primitive: 'Text', props: { children: 'Ready' } }]
      },
      outputs: {},
      compiledArtifact: expect.objectContaining({
        format: '1flowbase/js-block-compiled-artifact',
        program: expect.objectContaining({
          executableBody: expect.not.stringContaining('mutated artifact')
        })
      }),
      schemaValidationOptions: {
        maxDepth: 8,
        maxNodes: 250,
        allowedDataPermissions: ['query'],

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
      interfaceCalls: [],
      mediatorState: {
        eventChains: {
          'restricted-block:block-1:code-1::restricted-block:block-1:code-1': 1
        }
      }
    });
  });
});
