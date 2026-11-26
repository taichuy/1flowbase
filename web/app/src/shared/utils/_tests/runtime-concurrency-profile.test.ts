import { describe, expect, test } from 'vitest';

import { resolveRuntimeConcurrencyProfile } from '../runtime-concurrency-profile';

describe('resolveRuntimeConcurrencyProfile', () => {
  test('keeps conservative single-channel limits in DEV to protect frpc / unbundled ESM tunnels', () => {
    const profile = resolveRuntimeConcurrencyProfile({
      isDev: true,
      hardwareConcurrency: 16
    });
    expect(profile).toEqual({
      isDev: true,
      hardwareCores: 16,
      prewarmConcurrencyLimit: 1,
      removePendingPrewarmOnSubmit: true,
      compilerMaxWorkers: 1,
      compilerCacheGateInitialWorker: false,
      canvasPreparationConcurrency: 4,
      maxStartupPrefetchBlocks: 12,
      startupPrewarmSlotLimit: 1,
      parallelStartupAssetAndControl: false
    });
  });

  test('uncaps static bundle network prewarm in PROD while bounding Worker cores to clamp(floor(H/2), 2, 4) and viewport prewarm to 4 slots', () => {
    const eightCore = resolveRuntimeConcurrencyProfile({
      isDev: false,
      hardwareConcurrency: 8
    });
    expect(eightCore).toEqual({
      isDev: false,
      hardwareCores: 8,
      prewarmConcurrencyLimit: Number.POSITIVE_INFINITY,
      removePendingPrewarmOnSubmit: false,
      compilerMaxWorkers: 4,
      compilerCacheGateInitialWorker: true,
      canvasPreparationConcurrency: 6,
      maxStartupPrefetchBlocks: Number.POSITIVE_INFINITY,
      startupPrewarmSlotLimit: 4,
      parallelStartupAssetAndControl: true
    });

    const dualCore = resolveRuntimeConcurrencyProfile({
      isDev: false,
      hardwareConcurrency: 2
    });
    expect(dualCore.compilerMaxWorkers).toBe(2);
    expect(dualCore.canvasPreparationConcurrency).toBe(4);

    const sixCore = resolveRuntimeConcurrencyProfile({
      isDev: false,
      hardwareConcurrency: 6
    });
    expect(sixCore.compilerMaxWorkers).toBe(3);
    expect(sixCore.canvasPreparationConcurrency).toBe(5);

    const manyCore = resolveRuntimeConcurrencyProfile({
      isDev: false,
      hardwareConcurrency: 32
    });
    expect(manyCore.compilerMaxWorkers).toBe(4);
    expect(manyCore.canvasPreparationConcurrency).toBe(6);
  });
});
