import { describe, expect, test, vi } from 'vitest';

import {
  NATIVE_TRUSTED_BLOCK_PERMISSION,
  NATIVE_TRUSTED_BLOCK_RUNTIME,
  createNativeTrustedBlockHost,
  type NativeTrustedBlockHostAdapter,
  type NativeTrustedBlockMountedInstance,
  type NativeTrustedBlockPreparePlan
} from '../index';

function createPreparePlan(
  overrides: Partial<NativeTrustedBlockPreparePlan> = {}
): NativeTrustedBlockPreparePlan {
  return {
    runtime: NATIVE_TRUSTED_BLOCK_RUNTIME,
    blockId: 'block-1',
    entry: './Block.tsx',
    source: 'export default function Block() { return null; }',
    normalizedSource: 'export default function Block() { return null; }',
    props: { recordId: 'record-1' },
    requiredPermissions: [NATIVE_TRUSTED_BLOCK_PERMISSION],
    ...overrides
  };
}

describe('Native trusted block host lifecycle adapter', () => {
  test('mount delegates the prepare plan and caller root to the injected adapter', async () => {
    const plan = createPreparePlan();
    const root = { handle: 'root-1' };
    const mountedInstance = { dispose: vi.fn() };
    const adapter: NativeTrustedBlockHostAdapter = {
      mount: vi.fn().mockResolvedValue(mountedInstance)
    };
    const host = createNativeTrustedBlockHost({ adapter });

    const state = await host.mount(plan, root);

    expect(adapter.mount).toHaveBeenCalledTimes(1);
    expect(adapter.mount).toHaveBeenCalledWith({ plan, root });
    expect(state).toMatchObject({
      status: 'mounted',
      blockId: 'block-1',
      runtime: NATIVE_TRUSTED_BLOCK_RUNTIME
    });
    expect(host.getState()).toBe(state);
  });

  test('maps adapter mount failures to structured runtime errors scoped to the session', async () => {
    const adapter: NativeTrustedBlockHostAdapter = {
      mount: vi.fn().mockRejectedValue(new Error('mount exploded'))
    };
    const host = createNativeTrustedBlockHost({ adapter });

    const state = await host.mount(
      createPreparePlan({ blockId: 'block-fail' }),
      {
        handle: 'root-fail'
      }
    );

    expect(state).toMatchObject({
      status: 'failed',
      blockId: 'block-fail',
      runtime: NATIVE_TRUSTED_BLOCK_RUNTIME,
      error: {
        code: 'runtime_error',
        path: 'runtime.mount',
        message: expect.stringContaining('mount exploded')
      }
    });
  });

  test('dispose calls the mounted instance dispose exactly once and is idempotent', async () => {
    const dispose = vi.fn();
    const adapter: NativeTrustedBlockHostAdapter = {
      mount: vi.fn().mockReturnValue({ dispose })
    };
    const host = createNativeTrustedBlockHost({ adapter });

    await host.mount(createPreparePlan(), { handle: 'root-1' });
    const firstState = await host.dispose();
    const secondState = await host.dispose();

    expect(dispose).toHaveBeenCalledTimes(1);
    expect(firstState).toEqual({ status: 'disposed' });
    expect(secondState).toBe(firstState);
  });

  test('D3-AC-003 updates the mounted instance without a second adapter mount', async () => {
    const update = vi.fn();
    const adapter: NativeTrustedBlockHostAdapter = {
      mount: vi.fn().mockReturnValue({ update, dispose: vi.fn() })
    };
    const host = createNativeTrustedBlockHost({ adapter });
    const initial = createPreparePlan({ props: { count: 1 } });
    const changed = createPreparePlan({ props: { count: 2 } });

    await host.mount(initial, { handle: 'root-1' });
    const state = await host.update(changed);

    expect(adapter.mount).toHaveBeenCalledOnce();
    expect(update).toHaveBeenCalledOnce();
    expect(update).toHaveBeenCalledWith(changed);
    expect(state).toMatchObject({ status: 'mounted', blockId: 'block-1' });
  });

  test('D1-AC-004 retry disposes and remounts only the current block instance', async () => {
    const firstDispose = vi.fn();
    const secondDispose = vi.fn();
    const plan = createPreparePlan({ blockId: 'retry-block' });
    const root = { handle: 'retry-root' };
    const adapter: NativeTrustedBlockHostAdapter = {
      mount: vi
        .fn()
        .mockReturnValueOnce({ dispose: firstDispose })
        .mockReturnValueOnce({ dispose: secondDispose })
    };
    const host = createNativeTrustedBlockHost({ adapter });

    await host.mount(plan, root);
    const retriedState = await host.retry();

    expect(firstDispose).toHaveBeenCalledTimes(1);
    expect(adapter.mount).toHaveBeenCalledTimes(2);
    expect(adapter.mount).toHaveBeenNthCalledWith(2, { plan, root });
    expect(retriedState).toMatchObject({
      status: 'mounted',
      blockId: 'retry-block'
    });

    await host.dispose();
    expect(secondDispose).toHaveBeenCalledTimes(1);
  });

  test('disposes instances that resolve after dispose while mount is pending', async () => {
    const plan = createPreparePlan();
    const root = { handle: 'root-1' };
    const dispose = vi.fn();
    let resolveMount!: (instance: NativeTrustedBlockMountedInstance) => void;
    const pendingMount = new Promise<NativeTrustedBlockMountedInstance>(
      (resolve) => {
        resolveMount = resolve;
      }
    );
    const adapter: NativeTrustedBlockHostAdapter = {
      mount: vi.fn().mockReturnValue(pendingMount)
    };
    const host = createNativeTrustedBlockHost({ adapter });

    const mountStatePromise = host.mount(plan, root);
    const disposedState = await host.dispose();
    resolveMount({ dispose });
    const mountState = await mountStatePromise;

    expect(dispose).toHaveBeenCalledTimes(1);
    expect(disposedState).toEqual({ status: 'disposed' });
    expect(mountState).toBe(disposedState);
    expect(host.getState()).toBe(disposedState);
  });

  test('calls after dispose do not remount or reach mounted state', async () => {
    const adapter: NativeTrustedBlockHostAdapter = {
      mount: vi.fn()
    };
    const host = createNativeTrustedBlockHost({ adapter });

    const disposedState = await host.dispose();
    const mountState = await host.mount(createPreparePlan(), {
      handle: 'root-1'
    });
    const retryState = await host.retry();

    expect(adapter.mount).not.toHaveBeenCalled();
    expect(disposedState).toEqual({ status: 'disposed' });
    expect(mountState).toBe(disposedState);
    expect(retryState).toBe(disposedState);
    expect(host.getState().status).toBe('disposed');
  });
});
