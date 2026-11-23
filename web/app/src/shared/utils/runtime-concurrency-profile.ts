export interface RuntimeConcurrencyEnvironment {
  isDev?: boolean;
  hardwareConcurrency?: number;
}

export interface RuntimeConcurrencyProfile {
  readonly isDev: boolean;
  readonly hardwareCores: number;
  readonly prewarmConcurrencyLimit: number;
  readonly removePendingPrewarmOnSubmit: boolean;
  readonly compilerMaxWorkers: number;
  readonly compilerCacheGateInitialWorker: boolean;
  readonly canvasPreparationConcurrency: number;
  readonly maxStartupPrefetchBlocks: number;
  readonly startupPrewarmSlotLimit: number;
  readonly parallelStartupAssetAndControl: boolean;
}

export function resolveRuntimeConcurrencyProfile(
  env: RuntimeConcurrencyEnvironment = {}
): RuntimeConcurrencyProfile {
  const isDev = env.isDev ?? Boolean(import.meta.env.DEV);
  const raw =
    env.hardwareConcurrency ??
    (typeof navigator !== 'undefined'
      ? navigator.hardwareConcurrency
      : undefined);
  const hardwareCores =
    typeof raw === 'number' && Number.isFinite(raw) && raw >= 1
      ? Math.floor(raw)
      : isDev
        ? 4
        : 6;
  const compilerMaxWorkers = isDev
    ? 1
    : Math.min(4, Math.max(2, Math.floor(hardwareCores / 2)));

  return {
    isDev,
    hardwareCores,
    prewarmConcurrencyLimit: isDev ? 1 : Number.POSITIVE_INFINITY,
    removePendingPrewarmOnSubmit: isDev,
    compilerMaxWorkers,
    compilerCacheGateInitialWorker: !isDev,
    canvasPreparationConcurrency: isDev ? 4 : compilerMaxWorkers + 2,
    maxStartupPrefetchBlocks: isDev ? 12 : Number.POSITIVE_INFINITY,
    startupPrewarmSlotLimit: isDev ? 1 : 4,
    parallelStartupAssetAndControl: !isDev
  };
}
