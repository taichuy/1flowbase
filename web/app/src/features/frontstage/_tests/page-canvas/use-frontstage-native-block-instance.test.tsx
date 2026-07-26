import { renderHook, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import type {
  NativeTrustedBlockHost,
  NativeTrustedBlockPreparePlan
} from '@1flowbase/page-runtime';
import type { BlockProtocolError } from '@1flowbase/page-protocol';

import { useFrontstageNativeBlockInstance } from '../../hooks/use-frontstage-native-block-instance';
import { createFrontstageUnavailableBlockContext } from '../../lib/native-trusted-block-react-adapter';
import type {
  FrontstageNativeInstanceMountIntent,
  FrontstageNativePreparedRuntime
} from '../../lib/page-canvas/native-runtime-preparation';

describe('useFrontstageNativeBlockInstance', () => {
  test('D3-AC-003 updates props on the same Host and remounts exactly once when identity changes', async () => {
    const hosts = createHostFactory();
    const epochs = createEpochOwner();
    const root = document.createElement('div');
    const initialPlan = plan({ title: 'Initial' });
    const { rerender } = renderHook(
      ({ mountIntent, runtimePlan }) =>
        useFrontstageNativeBlockInstance({
          root,
          mountIntent,
          prepared: prepared(),
          createRuntimeInput: () => ({
            plan: runtimePlan,
            context: createFrontstageUnavailableBlockContext(runtimePlan)
          }),
          instanceEpochOwner: epochs.owner,
          hostFactory: hosts.factory
        }),
      {
        initialProps: {
          mountIntent: intent('source-a'),
          runtimePlan: initialPlan
        }
      }
    );
    await waitFor(() => expect(hosts.mount).toHaveBeenCalledOnce());

    const changedPropsPlan = plan({ title: 'Changed' });
    rerender({
      mountIntent: intent('source-a'),
      runtimePlan: changedPropsPlan
    });
    await waitFor(() => expect(hosts.update).toHaveBeenCalledOnce());
    expect(hosts.factory).toHaveBeenCalledOnce();
    expect(hosts.dispose).not.toHaveBeenCalled();
    expect(epochs.begin).toHaveBeenCalledOnce();

    rerender({
      mountIntent: intent('source-b'),
      runtimePlan: changedPropsPlan
    });
    await waitFor(() => expect(hosts.factory).toHaveBeenCalledTimes(2));
    expect(hosts.dispose).toHaveBeenCalledOnce();
    expect(hosts.mount).toHaveBeenCalledTimes(2);
    expect(epochs.begin).toHaveBeenCalledTimes(2);
    expect(epochs.end).toHaveBeenCalledWith('epoch-1');
  });

  test('D3-AC-004 unmounts for demand 1 to 2, remounts for 2 to 1, and disposes on page leave', async () => {
    const hosts = createHostFactory();
    const epochs = createEpochOwner();
    const root = document.createElement('div');
    const runtimePlan = plan({ title: 'Demand' });
    const renderInstance = (
      mountIntent: FrontstageNativeInstanceMountIntent | null
    ) =>
      useFrontstageNativeBlockInstance({
        root,
        mountIntent,
        prepared: prepared(),
        createRuntimeInput: () => ({
          plan: runtimePlan,
          context: createFrontstageUnavailableBlockContext(runtimePlan)
        }),
        instanceEpochOwner: epochs.owner,
        hostFactory: hosts.factory
      });
    const { rerender, unmount } = renderHook(
      ({
        mountIntent
      }: {
        mountIntent: FrontstageNativeInstanceMountIntent | null;
      }) => renderInstance(mountIntent),
      {
        initialProps: {
          mountIntent: intent(
            'source-a'
          ) as FrontstageNativeInstanceMountIntent | null
        }
      }
    );
    await waitFor(() => expect(hosts.mount).toHaveBeenCalledOnce());

    rerender({ mountIntent: null });
    await waitFor(() => expect(hosts.dispose).toHaveBeenCalledOnce());
    rerender({ mountIntent: intent('source-a') });
    await waitFor(() => expect(hosts.mount).toHaveBeenCalledTimes(2));
    expect(epochs.begin).toHaveBeenNthCalledWith(1);
    expect(epochs.begin).toHaveBeenNthCalledWith(2);
    expect(epochs.end).toHaveBeenCalledWith('epoch-1');

    Object.defineProperty(document, 'visibilityState', {
      configurable: true,
      value: 'hidden'
    });
    document.dispatchEvent(new Event('visibilitychange'));
    expect(hosts.dispose).toHaveBeenCalledOnce();

    unmount();
    await waitFor(() => expect(hosts.dispose).toHaveBeenCalledTimes(2));
    expect(epochs.end).toHaveBeenCalledWith('epoch-2');
  });

  test('D3-AC-006 render retry remounts only the failed identity with a new epoch', async () => {
    const hosts = createHostFactory();
    const epochs = createEpochOwner();
    const root = document.createElement('div');
    const runtimePlan = plan({ title: 'Retry' });
    const { result } = renderHook(() =>
      useFrontstageNativeBlockInstance({
        root,
        mountIntent: intent('source-a'),
        prepared: prepared(),
        createRuntimeInput: () => ({
          plan: runtimePlan,
          context: createFrontstageUnavailableBlockContext(runtimePlan)
        }),
        instanceEpochOwner: epochs.owner,
        hostFactory: hosts.factory
      })
    );
    await waitFor(() => expect(hosts.mount).toHaveBeenCalledOnce());
    hosts.fail({
      code: 'runtime_error',
      path: 'runtime.render',
      message: 'controlled render failure'
    });
    await waitFor(() => expect(result.current.status).toBe('failed'));

    result.current.retry();
    await waitFor(() => expect(hosts.mount).toHaveBeenCalledTimes(2));
    expect(hosts.dispose).toHaveBeenCalledOnce();
    expect(hosts.factory).toHaveBeenCalledTimes(2);
    expect(epochs.begin).toHaveBeenCalledTimes(2);
    expect(epochs.end).toHaveBeenCalledWith('epoch-1');
  });
});

function createHostFactory() {
  let onRuntimeError: ((error: BlockProtocolError) => void) | undefined;
  const mount = vi.fn(async (runtimePlan: NativeTrustedBlockPreparePlan) => ({
    status: 'mounted' as const,
    blockId: runtimePlan.blockId,
    runtime: runtimePlan.runtime
  }));
  const update = vi.fn(async (runtimePlan: NativeTrustedBlockPreparePlan) => ({
    status: 'mounted' as const,
    blockId: runtimePlan.blockId,
    runtime: runtimePlan.runtime
  }));
  const dispose = vi.fn(async () => ({ status: 'disposed' as const }));
  const factory = vi.fn(
    (input: {
      onRuntimeError(error: BlockProtocolError): void;
    }): NativeTrustedBlockHost => {
      onRuntimeError = input.onRuntimeError;
      return {
        getState: () => ({ status: 'idle' }),
        mount,
        update,
        retry: vi.fn(async () => ({ status: 'mounted' as const })),
        dispose
      };
    }
  );
  return {
    factory,
    mount,
    update,
    dispose,
    fail(error: BlockProtocolError) {
      onRuntimeError?.(error);
    }
  };
}

function createEpochOwner() {
  let nextEpoch = 0;
  const begin = vi.fn(() => `epoch-${++nextEpoch}`);
  const end = vi.fn();
  return { owner: { begin, end }, begin, end };
}

function intent(sourceSha256: string): FrontstageNativeInstanceMountIntent {
  return {
    blockId: 'block-1',
    slotIndex: 0,
    identityInput: {
      sourceSha256,
      runtimeFingerprint: 'runtime-a',
      dependencyLockIdentity: 'lock-a'
    }
  };
}

function prepared(): FrontstageNativePreparedRuntime {
  return {
    artifact: {} as FrontstageNativePreparedRuntime['artifact'],
    component: (() => null) as FrontstageNativePreparedRuntime['component'],
    identityInput: intent('source-a').identityInput,
    artifactCacheTier: 'l2'
  };
}

function plan(props: Record<string, unknown>): NativeTrustedBlockPreparePlan {
  return {
    runtime: 'native_trusted_block',
    blockId: 'block-1',
    entry: 'default',
    source: '/* prepared */',
    normalizedSource: '/* prepared */',
    props,
    requiredPermissions: ['ui_block.javascript.native']
  };
}
