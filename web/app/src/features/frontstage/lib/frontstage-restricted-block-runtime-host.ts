import type {
  JsBlockWorkerFactory,
  JsBlockWorkerLike,
  JsBlockRunPhase,
  JsBlockWorkerScheduleTimeout
} from '@1flowbase/page-runtime';

import {
  createRestrictedBlockRuntimeHost,
  type RestrictedBlockRuntimeHost,
  type RestrictedBlockRuntimeHostOptions,
  type RestrictedBlockRuntimeHostSnapshot
} from './restricted-block-runtime-host';
import {
  createFrontstageRestrictedBlockWorkerFactory,
  getFrontstageRestrictedBlockRuntimeFingerprint,
  getFrontstageRestrictedBlockWorkerUrl,
  type FrontstageRestrictedBlockWorkerFactoryOptions
} from './restricted-block-worker-factory';

export interface FrontstageRestrictedBlockRuntimeHostOptions
  extends Omit<RestrictedBlockRuntimeHostOptions, 'workerFactory'> {
  workerFactory?: JsBlockWorkerFactory;
  browserWorkerFactoryOptions?: FrontstageRestrictedBlockWorkerFactoryOptions;
}

export type FrontstageRestrictedBlockRuntimeSnapshotListener = (
  snapshot: RestrictedBlockRuntimeHostSnapshot
) => void;

export interface ObservableFrontstageRestrictedBlockRuntimeHost
  extends RestrictedBlockRuntimeHost {
  subscribe(
    listener: FrontstageRestrictedBlockRuntimeSnapshotListener
  ): () => void;
}

export type FrontstageRestrictedBlockRuntimeSession =
  ObservableFrontstageRestrictedBlockRuntimeHost;

export function createFrontstageRestrictedBlockRuntimeHost(
  options: FrontstageRestrictedBlockRuntimeHostOptions
): RestrictedBlockRuntimeHost {
  const { browserWorkerFactoryOptions, workerFactory, ...runtimeOptions } =
    options;

  return createRestrictedBlockRuntimeHost({
    ...runtimeOptions,
    runtimeFingerprint:
      runtimeOptions.runtimeFingerprint ??
      getFrontstageRestrictedBlockRuntimeFingerprint(
        browserWorkerFactoryOptions?.workerUrl ??
          getFrontstageRestrictedBlockWorkerUrl()
      ),
    workerFactory:
      workerFactory ??
      createFrontstageRestrictedBlockWorkerFactory(browserWorkerFactoryOptions)
  });
}

export function createObservableFrontstageRestrictedBlockRuntimeHost(
  options: FrontstageRestrictedBlockRuntimeHostOptions
): ObservableFrontstageRestrictedBlockRuntimeHost {
  const listeners = new Set<FrontstageRestrictedBlockRuntimeSnapshotListener>();

  const notifySnapshotChange = (phaseOverride?: JsBlockRunPhase) => {
    for (const listener of [...listeners]) {
      if (listeners.has(listener)) {
        const snapshot = host.getSnapshot();
        listener(
          phaseOverride && snapshot.status === 'running'
            ? { ...snapshot, phase: phaseOverride }
            : snapshot
        );
      }
    }
  };

  const {
    browserWorkerFactoryOptions,
    workerFactory,
    scheduleTimeout,
    ...runtimeOptions
  } = options;

  const host = createRestrictedBlockRuntimeHost({
    ...runtimeOptions,
    runtimeFingerprint:
      runtimeOptions.runtimeFingerprint ??
      getFrontstageRestrictedBlockRuntimeFingerprint(
        browserWorkerFactoryOptions?.workerUrl ??
          getFrontstageRestrictedBlockWorkerUrl()
      ),
    scheduleTimeout: createNotifyingScheduleTimeout(
      scheduleTimeout,
      notifySnapshotChange
    ),
    workerFactory: createNotifyingWorkerFactory(
      workerFactory ??
        createFrontstageRestrictedBlockWorkerFactory(browserWorkerFactoryOptions),
      notifySnapshotChange,
      () => notifySnapshotChange('validating_schema')
    )
  });

  return {
    run() {
      const snapshot = host.run();
      notifySnapshotChange();
      return snapshot;
    },
    dispose() {
      const snapshot = host.dispose();
      notifySnapshotChange();
      return snapshot;
    },
    getSnapshot() {
      return host.getSnapshot();
    },
    getHostState() {
      return host.getHostState();
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    }
  };
}

export function createFrontstageRestrictedBlockRuntimeSession(
  options: FrontstageRestrictedBlockRuntimeHostOptions
): FrontstageRestrictedBlockRuntimeSession {
  return createObservableFrontstageRestrictedBlockRuntimeHost(options);
}

function createNotifyingWorkerFactory(
  workerFactory: JsBlockWorkerFactory,
  notifySnapshotChange: () => void,
  notifySchemaValidation: () => void
): JsBlockWorkerFactory {
  return () =>
    new SnapshotNotifyingJsBlockWorker(
      workerFactory(),
      notifySnapshotChange,
      notifySchemaValidation
    );
}

function createNotifyingScheduleTimeout(
  scheduleTimeout: JsBlockWorkerScheduleTimeout | undefined,
  notifySnapshotChange: () => void
): JsBlockWorkerScheduleTimeout {
  const resolvedScheduleTimeout =
    scheduleTimeout ??
    ((callback, timeoutMs) => setTimeout(callback, timeoutMs));

  return (callback, timeoutMs) =>
    resolvedScheduleTimeout(() => {
      callback();
      notifySnapshotChange();
    }, timeoutMs);
}

type JsBlockWorkerMessageHandler = NonNullable<
  JsBlockWorkerLike['onmessage']
>;
type JsBlockWorkerErrorHandler = NonNullable<JsBlockWorkerLike['onerror']>;
type JsBlockWorkerMessageErrorHandler = NonNullable<
  JsBlockWorkerLike['onmessageerror']
>;

class SnapshotNotifyingJsBlockWorker implements JsBlockWorkerLike {
  private messageHandler: JsBlockWorkerMessageHandler | null = null;
  private errorHandler: JsBlockWorkerErrorHandler | null = null;
  private messageErrorHandler: JsBlockWorkerMessageErrorHandler | null = null;

  constructor(
    private readonly worker: JsBlockWorkerLike,
    private readonly notifySnapshotChange: () => void,
    private readonly notifySchemaValidation: () => void
  ) {}

  get onmessage(): JsBlockWorkerMessageHandler | null {
    return this.messageHandler;
  }

  set onmessage(handler: JsBlockWorkerMessageHandler | null) {
    this.messageHandler = handler;
    this.worker.onmessage =
      handler === null
        ? null
        : (event) => {
            try {
              if (isCompletedWorkerMessage(event.data)) {
                this.notifySchemaValidation();
              }
              handler(event);
            } finally {
              this.notifySnapshotChange();
            }
          };
  }

  get onerror(): JsBlockWorkerErrorHandler | null {
    return this.errorHandler;
  }

  set onerror(handler: JsBlockWorkerErrorHandler | null) {
    this.errorHandler = handler;
    this.worker.onerror =
      handler === null
        ? null
        : (event) => {
            try {
              handler(event);
            } finally {
              this.notifySnapshotChange();
            }
          };
  }

  get onmessageerror(): JsBlockWorkerMessageErrorHandler | null {
    return this.messageErrorHandler;
  }

  set onmessageerror(handler: JsBlockWorkerMessageErrorHandler | null) {
    this.messageErrorHandler = handler;
    this.worker.onmessageerror =
      handler === null
        ? null
        : (event) => {
            try {
              handler(event);
            } finally {
              this.notifySnapshotChange();
            }
          };
  }

  postMessage(message: Parameters<JsBlockWorkerLike['postMessage']>[0]): void {
    this.worker.postMessage(message);
  }

  terminate(): void {
    this.worker.terminate();
  }
}

function isCompletedWorkerMessage(value: unknown): boolean {
  return (
    typeof value === 'object' &&
    value !== null &&
    (value as { direction?: unknown }).direction === 'worker_to_host' &&
    (value as { type?: unknown }).type === 'completed'
  );
}
