import {
  createJsBlockBrowserWorkerFactory,
  type JsBlockBrowserWorkerConstructor,
  type JsBlockBrowserWorkerFactoryOptions,
  type JsBlockWorkerFactory,
  type JsBlockWorkerLike
} from '@1flowbase/page-runtime';

import frontstageRestrictedBlockWorkerUrl from '../workers/restricted-block-runtime.worker?worker&url';

export const FRONTSTAGE_RESTRICTED_BLOCK_WORKER_NAME =
  'frontstage-restricted-block-runtime';

export interface FrontstageRestrictedBlockWorkerFactoryOptions {
  workerConstructor?: JsBlockBrowserWorkerConstructor | null;
  workerUrl?: string | URL | null;
  workerOptions?: WorkerOptions;
}

const WARM_WORKER_TTL_MS = 30_000;
let warmWorker: JsBlockWorkerLike | null = null;
let warmWorkerTimer: ReturnType<typeof setTimeout> | null = null;
let visibilityCleanupInstalled = false;

export function getFrontstageRestrictedBlockWorkerUrl(): string {
  return frontstageRestrictedBlockWorkerUrl;
}

export function getFrontstageRestrictedBlockWorkerOptions(
  overrides: WorkerOptions = {}
): WorkerOptions {
  return {
    type: 'module',
    name: FRONTSTAGE_RESTRICTED_BLOCK_WORKER_NAME,
    ...overrides
  };
}

export function createFrontstageRestrictedBlockWorkerFactory(
  options: FrontstageRestrictedBlockWorkerFactoryOptions = {}
): JsBlockWorkerFactory {
  const factoryOptions: JsBlockBrowserWorkerFactoryOptions = {
    workerUrl: Object.hasOwn(options, 'workerUrl')
      ? options.workerUrl
      : getFrontstageRestrictedBlockWorkerUrl(),
    workerOptions: getFrontstageRestrictedBlockWorkerOptions(
      options.workerOptions
    )
  };

  if (Object.hasOwn(options, 'workerConstructor')) {
    factoryOptions.workerConstructor = options.workerConstructor;
  }

  const createWorker = createJsBlockBrowserWorkerFactory(factoryOptions);
  const canUseSharedWarmLease =
    !Object.hasOwn(options, 'workerConstructor') &&
    !Object.hasOwn(options, 'workerUrl') &&
    !Object.hasOwn(options, 'workerOptions');

  if (!canUseSharedWarmLease) {
    return createWorker;
  }

  installWarmWorkerVisibilityCleanup();
  return () => {
    const worker = warmWorker ?? createWorker();
    if (warmWorker === worker) {
      warmWorker = null;
      clearWarmWorkerTimer();
    }
    scheduleWarmWorker(createWorker);
    return worker;
  };
}

function scheduleWarmWorker(createWorker: JsBlockWorkerFactory): void {
  if (
    warmWorker ||
    warmWorkerTimer ||
    (typeof document !== 'undefined' && document.visibilityState === 'hidden')
  ) {
    return;
  }
  warmWorkerTimer = setTimeout(() => {
    warmWorkerTimer = null;
    if (
      warmWorker ||
      (typeof document !== 'undefined' && document.visibilityState === 'hidden')
    ) {
      return;
    }
    warmWorker = createWorker();
    warmWorkerTimer = setTimeout(releaseWarmWorker, WARM_WORKER_TTL_MS);
  }, 0);
}

function releaseWarmWorker(): void {
  clearWarmWorkerTimer();
  warmWorker?.terminate();
  warmWorker = null;
}

function clearWarmWorkerTimer(): void {
  if (warmWorkerTimer) {
    clearTimeout(warmWorkerTimer);
    warmWorkerTimer = null;
  }
}

function installWarmWorkerVisibilityCleanup(): void {
  if (
    visibilityCleanupInstalled ||
    typeof document === 'undefined'
  ) {
    return;
  }
  visibilityCleanupInstalled = true;
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') {
      releaseWarmWorker();
    }
  });
}
