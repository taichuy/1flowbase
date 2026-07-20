import { describe, expect, test, vi } from 'vitest';

import {
  JsBlockWorkerAdapterError,
  type JsBlockRunRequest,
  type JsBlockWorkerLike
} from '@1flowbase/page-runtime';

import type { RestrictedBlockRunPlan } from '../../lib/restricted-block-loader';
import {
  createObservableFrontstageRestrictedBlockRuntimeHost,
  createFrontstageRestrictedBlockRuntimeHost,
  type FrontstageRestrictedBlockRuntimeHostOptions
} from '../../lib/frontstage-restricted-block-runtime-host';
import { getFrontstageRestrictedBlockWorkerUrl } from '../../lib/restricted-block-worker-factory';

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

  emitMessageError(message = 'worker message failed'): void {
    this.onmessageerror?.({ message });
  }
}

class FakeNativeWorker {
  static readonly instances: FakeNativeWorker[] = [];

  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: ErrorEvent) => void) | null = null;
  onmessageerror: ((event: MessageEvent) => void) | null = null;
  readonly scriptUrl: string | URL;
  readonly options?: WorkerOptions;
  readonly messages: unknown[] = [];
  terminateCount = 0;

  constructor(scriptUrl: string | URL, options?: WorkerOptions) {
    this.scriptUrl = scriptUrl;
    this.options = options;
    FakeNativeWorker.instances.push(this);
  }

  postMessage(message: unknown): void {
    this.messages.push(message);
  }

  terminate(): void {
    this.terminateCount += 1;
  }
}

class ThrowingNativeWorker extends FakeNativeWorker {
  constructor(scriptUrl: string | URL, options?: WorkerOptions) {
    super(scriptUrl, options);
    throw new Error('native worker blocked');
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

      allowedEvents: ['record.saved']
    },
    mediatorPolicy: {
      allowedEvents: ['record.saved'],

      allowedInterfaces: ['listRecords'],

      maxEventChainDepth: 4
    },
    ...overrides
  };
}

function createSubject(
  options: Partial<FrontstageRestrictedBlockRuntimeHostOptions> = {}
) {
  const worker = new FakeWorker();
  const host = createFrontstageRestrictedBlockRuntimeHost({
    runPlan: createRunPlan(),
    workerFactory: () => worker,
    ...options
  });

  return { host, worker };
}

function createObservableSubject(
  options: Partial<FrontstageRestrictedBlockRuntimeHostOptions> = {}
) {
  const worker = new FakeWorker();
  const host = createObservableFrontstageRestrictedBlockRuntimeHost({
    runPlan: createRunPlan(),
    workerFactory: () => worker,
    ...options
  });

  return { host, worker };
}

describe('FrontStage restricted block runtime host factory', () => {
  test('notifies subscribers with running and ready snapshots', () => {
    const { host, worker } = createObservableSubject();
    const snapshots: Array<ReturnType<typeof host.getSnapshot>> = [];

    host.subscribe((snapshot) => snapshots.push(snapshot));

    host.run();
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'completed',
      requestId: 'restricted-block:block-1:code-1',
      view: { primitive: 'Text', props: { children: 'Ready' } },
      outputs: {}
    });

    expect(snapshots).toMatchObject([
      { status: 'running' },
      {
        status: 'ready',
        view: { primitive: 'Text', props: { children: 'Ready' } },
        outputs: {}
      }
    ]);
  });

  test('notifies subscribers with failed snapshots after worker errors', () => {
    const failedByError = createObservableSubject();
    const errorSnapshots: Array<
      ReturnType<typeof failedByError.host.getSnapshot>
    > = [];

    failedByError.host.subscribe((snapshot) => errorSnapshots.push(snapshot));
    failedByError.host.run();
    failedByError.worker.emitError('worker exploded');

    expect(errorSnapshots).toMatchObject([
      { status: 'running' },
      {
        status: 'failed',
        error: { kind: 'worker_crash', message: 'worker exploded' }
      }
    ]);

    const failedByMessageError = createObservableSubject();
    const messageErrorSnapshots: Array<
      ReturnType<typeof failedByMessageError.host.getSnapshot>
    > = [];

    failedByMessageError.host.subscribe((snapshot) =>
      messageErrorSnapshots.push(snapshot)
    );
    failedByMessageError.host.run();
    failedByMessageError.worker.emitMessageError('message channel exploded');

    expect(messageErrorSnapshots).toMatchObject([
      { status: 'running' },
      {
        status: 'failed',
        error: {
          kind: 'worker_crash',
          message: 'message channel exploded'
        }
      }
    ]);
  });

  test('notifies subscribers after dispose', () => {
    const { host, worker } = createObservableSubject();
    const snapshots: Array<ReturnType<typeof host.getSnapshot>> = [];

    host.subscribe((snapshot) => snapshots.push(snapshot));
    host.run();

    expect(host.dispose()).toMatchObject({ status: 'disposed' });
    expect(snapshots).toMatchObject([
      { status: 'running' },
      { status: 'disposed' }
    ]);
    expect(worker.terminateCount).toBe(1);
  });

  test('ignores late worker messages after dispose without notifying stale snapshots', () => {
    const { host, worker } = createObservableSubject();
    const snapshots: Array<ReturnType<typeof host.getSnapshot>> = [];

    host.subscribe((snapshot) => snapshots.push(snapshot));
    host.run();
    host.dispose();
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'completed',
      requestId: 'restricted-block:block-1:code-1',
      view: { primitive: 'Text', props: { children: 'Late' } },
      outputs: {}
    });

    expect(snapshots).toMatchObject([
      { status: 'running' },
      { status: 'disposed' }
    ]);
    const snapshot = host.getSnapshot();
    expect(snapshot.status).toBe('disposed');
    expect(snapshot.view).toBeUndefined();
  });

  test('stops notifying after unsubscribe', () => {
    const { host, worker } = createObservableSubject();
    const snapshots: Array<ReturnType<typeof host.getSnapshot>> = [];
    const unsubscribe = host.subscribe((snapshot) => snapshots.push(snapshot));

    host.run();
    unsubscribe();
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'completed',
      requestId: 'restricted-block:block-1:code-1',
      view: { primitive: 'Text', props: { children: 'Ready' } },
      outputs: {}
    });
    host.dispose();

    expect(snapshots).toMatchObject([{ status: 'running' }]);
  });

  test('notifies each subscriber with an isolated cloned snapshot', () => {
    const { host, worker } = createObservableSubject();
    const mutateFirstSnapshot = vi.fn(
      (snapshot: ReturnType<typeof host.getSnapshot>) => {
        if (snapshot.status !== 'ready') {
          return;
        }

        const schema = snapshot.view as {
          props: { children: string };
        };
        schema.props.children = 'Mutated';
        (
          snapshot.schemaValidationOptions.allowedActions as
            | string[]
            | undefined
        )?.push('record.delete');
      }
    );
    const secondSnapshots: Array<ReturnType<typeof host.getSnapshot>> = [];

    host.subscribe(mutateFirstSnapshot);
    host.subscribe((snapshot) => secondSnapshots.push(snapshot));
    host.run();
    worker.emitMessage({
      direction: 'worker_to_host',
      type: 'completed',
      requestId: 'restricted-block:block-1:code-1',
      view: { primitive: 'Text', props: { children: 'Ready' } },
      outputs: {}
    });

    expect(mutateFirstSnapshot).toHaveBeenCalledTimes(2);
    expect(secondSnapshots[1]).toMatchObject({
      status: 'ready',
      view: { primitive: 'Text', props: { children: 'Ready' } },
      outputs: {},
      schemaValidationOptions: {}
    });
    expect(host.getSnapshot()).toMatchObject({
      status: 'ready',
      view: { primitive: 'Text', props: { children: 'Ready' } },
      outputs: {},
      schemaValidationOptions: {}
    });
  });

  test('observable host uses an injected workerFactory instead of browser Worker options', () => {
    FakeNativeWorker.instances.length = 0;
    const worker = new FakeWorker();

    const host = createObservableFrontstageRestrictedBlockRuntimeHost({
      runPlan: createRunPlan(),
      workerFactory: () => worker,
      browserWorkerFactoryOptions: {
        workerConstructor: ThrowingNativeWorker
      }
    });

    host.run();

    expect(FakeNativeWorker.instances).toEqual([]);
    expect(worker.messages).toEqual([
      { direction: 'host_to_worker', type: 'init' }
    ]);
  });

  test('uses the FrontStage browser Worker factory by default', () => {
    FakeNativeWorker.instances.length = 0;

    createFrontstageRestrictedBlockRuntimeHost({
      runPlan: createRunPlan(),
      browserWorkerFactoryOptions: {
        workerConstructor: FakeNativeWorker
      }
    });

    expect(FakeNativeWorker.instances).toHaveLength(1);
    expect(String(FakeNativeWorker.instances[0]?.scriptUrl)).toBe(
      String(getFrontstageRestrictedBlockWorkerUrl())
    );
    expect(FakeNativeWorker.instances[0]?.options).toEqual({
      type: 'module',
      name: 'frontstage-restricted-block-runtime'
    });
  });

  test('uses an injected workerFactory instead of browser Worker options', () => {
    FakeNativeWorker.instances.length = 0;
    const worker = new FakeWorker();

    const host = createFrontstageRestrictedBlockRuntimeHost({
      runPlan: createRunPlan(),
      workerFactory: () => worker,
      browserWorkerFactoryOptions: {
        workerConstructor: ThrowingNativeWorker
      }
    });

    host.run();

    expect(FakeNativeWorker.instances).toEqual([]);
    expect(worker.messages).toEqual([
      { direction: 'host_to_worker', type: 'init' }
    ]);
  });

  test('keeps run and dispose snapshots aligned with the restricted runtime host', () => {
    const { host, worker } = createSubject();

    expect(host.run()).toMatchObject({
      status: 'running',
      requestId: 'restricted-block:block-1:code-1',
      blockId: 'block-1',
      schemaValidationOptions: {
        maxDepth: 8,
        maxNodes: 250,
        allowedDataPermissions: ['query'],

        allowedEvents: ['record.saved']
      },
      logs: [],
      effects: [],
      rejections: []
    });
    expect(host.dispose()).toMatchObject({
      status: 'disposed',
      requestId: 'restricted-block:block-1:code-1',
      blockId: 'block-1'
    });
    expect(worker.terminateCount).toBe(1);
  });

  test('passes through browser Worker construction errors with page-runtime attribution', () => {
    try {
      createFrontstageRestrictedBlockRuntimeHost({
        runPlan: createRunPlan(),
        browserWorkerFactoryOptions: {
          workerConstructor: ThrowingNativeWorker
        }
      });
      expect.unreachable('expected worker construction to fail');
    } catch (error) {
      expect(error).toBeInstanceOf(JsBlockWorkerAdapterError);
      expect(error).toMatchObject({
        code: 'worker_construct_failed',
        message: 'Failed to construct JS block worker: native worker blocked'
      });
    }
  });
});
