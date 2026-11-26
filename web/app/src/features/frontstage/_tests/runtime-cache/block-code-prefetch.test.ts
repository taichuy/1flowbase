import { describe, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  warmWorker: vi.fn(),
  prewarmModules: vi.fn()
}));

vi.mock(
  '../../../../shared/code-block/native-react-compiler-browser',
  () => ({
    warmDefaultNativeReactCompilerWorker: mocks.warmWorker
  })
);
vi.mock('../../lib/native-modules/registry', () => ({
  prewarmNativeModuleSources: mocks.prewarmModules
}));

import { resolveRuntimeConcurrencyProfile } from '../../../../shared/utils/runtime-concurrency-profile';
import { prewarmStartupBlockRuntime } from '../../lib/runtime-cache/block-code-prefetch';

describe('prewarmStartupBlockRuntime', () => {
  test('prewarms strictly 1 slot in DEV and strictly 4 first-screen slots in PROD when page has many blocks', async () => {
    const entries = Array.from({ length: 10 }, (_, index) => ({
      blockId: `block-${index}`,
      promise: Promise.resolve({
        block_id: `block-${index}`,
        page_id: 'page-1',
        source_code: `export default () => "slot-${index}";`,
        source_sha256: `sha-${index}`
      })
    }));

    mocks.warmWorker.mockClear();
    mocks.prewarmModules.mockClear();
    prewarmStartupBlockRuntime(
      entries,
      resolveRuntimeConcurrencyProfile({ isDev: true, hardwareConcurrency: 8 })
    );
    await Promise.resolve();
    await Promise.resolve();
    expect(mocks.warmWorker).not.toHaveBeenCalled();
    expect(mocks.prewarmModules.mock.calls.map(([src]) => src)).toEqual([
      'export default () => "slot-0";'
    ]);

    mocks.warmWorker.mockClear();
    mocks.prewarmModules.mockClear();
    prewarmStartupBlockRuntime(
      entries,
      resolveRuntimeConcurrencyProfile({ isDev: false, hardwareConcurrency: 8 })
    );
    await Promise.resolve();
    await Promise.resolve();
    expect(mocks.warmWorker).not.toHaveBeenCalled();
    expect(mocks.prewarmModules.mock.calls.map(([src]) => src)).toEqual([
      'export default () => "slot-0";',
      'export default () => "slot-1";',
      'export default () => "slot-2";',
      'export default () => "slot-3";'
    ]);
  });
});
