import type {
  BlockContext,
  BlockProtocolError
} from '@1flowbase/page-protocol';
import {
  createNativeTrustedBlockHost,
  type NativeTrustedBlockHost,
  type NativeTrustedBlockPreparePlan
} from '@1flowbase/page-runtime';
import type { ConfigProviderProps } from 'antd/es/config-provider';
import { useEffect, useMemo, useRef, useState } from 'react';

import {
  createFrontstageNativeTrustedBlockReactAdapter,
  type FrontstageNativeTrustedBlockReactComponent
} from '../lib/native-trusted-block-react-adapter';
import type {
  FrontstageNativeInstanceMountIntent,
  FrontstageNativePreparedRuntime
} from '../lib/page-canvas/native-runtime-preparation';

export type FrontstageNativeBlockInstanceStatus =
  | 'unmounted'
  | 'mounting'
  | 'mounted'
  | 'updating'
  | 'failed'
  | 'disposing';

export interface FrontstageNativeBlockInstanceState {
  status: FrontstageNativeBlockInstanceStatus;
  instanceEpoch?: string;
  error?: BlockProtocolError;
}

export interface FrontstageNativeBlockInstanceRuntimeInput {
  plan: NativeTrustedBlockPreparePlan;
  context: BlockContext;
  providerScope?: {
    theme?: ConfigProviderProps['theme'];
    locale?: ConfigProviderProps['locale'];
  };
}

export type FrontstageNativeBlockInstanceHostFactory = (input: {
  prepared: FrontstageNativePreparedRuntime;
  readRuntimeInput(): FrontstageNativeBlockInstanceRuntimeInput;
}) => NativeTrustedBlockHost;

export interface UseFrontstageNativeBlockInstanceInput {
  root: Element | null;
  mountIntent: FrontstageNativeInstanceMountIntent | null;
  prepared: FrontstageNativePreparedRuntime | null;
  createRuntimeInput(
    instanceEpoch: string
  ): FrontstageNativeBlockInstanceRuntimeInput;
  instanceEpochOwner?: FrontstageNativeInstanceEpochOwner;
  hostFactory?: FrontstageNativeBlockInstanceHostFactory;
}

interface ActiveNativeBlockInstance {
  identity: string;
  instanceEpoch: string;
  host: NativeTrustedBlockHost;
  mountedPlan: NativeTrustedBlockPreparePlan;
  mountedRuntimeInputFactory: UseFrontstageNativeBlockInstanceInput['createRuntimeInput'];
}

export interface FrontstageNativeInstanceEpochOwner {
  begin(): string;
  end(instanceEpoch: string): void;
}

let nextStandaloneInstanceEpoch = 0;

export function useFrontstageNativeBlockInstance({
  root,
  mountIntent,
  prepared,
  createRuntimeInput,
  instanceEpochOwner,
  hostFactory = createFrontstageNativeBlockInstanceHost
}: UseFrontstageNativeBlockInstanceInput): FrontstageNativeBlockInstanceState {
  const [state, setState] = useState<FrontstageNativeBlockInstanceState>({
    status: 'unmounted'
  });
  const activeRef = useRef<ActiveNativeBlockInstance | null>(null);
  const createRuntimeInputRef = useRef(createRuntimeInput);
  createRuntimeInputRef.current = createRuntimeInput;
  const lifecycleGenerationRef = useRef(0);
  const disposalRef = useRef<Promise<unknown>>(Promise.resolve());
  const identity = useMemo(
    () =>
      mountIntent ? nativeInstanceIdentity(mountIntent.identityInput) : null,
    [mountIntent]
  );

  useEffect(() => {
    const generation = ++lifecycleGenerationRef.current;
    if (!root || !identity || !prepared || !mountIntent) {
      setState({ status: 'unmounted' });
      return;
    }

    let cancelled = false;
    const mount = async () => {
      await disposalRef.current;
      if (cancelled || lifecycleGenerationRef.current !== generation) return;
      setState({ status: 'mounting' });
      const instanceEpoch =
        instanceEpochOwner?.begin() ??
        `standalone:${++nextStandaloneInstanceEpoch}`;
      const host = hostFactory({
        prepared,
        readRuntimeInput: () => createRuntimeInputRef.current(instanceEpoch)
      });
      const mountedRuntimeInputFactory = createRuntimeInputRef.current;
      const plan = mountedRuntimeInputFactory(instanceEpoch).plan;
      activeRef.current = {
        identity,
        instanceEpoch,
        host,
        mountedPlan: plan,
        mountedRuntimeInputFactory
      };
      const hostState = await host.mount(plan, root);
      if (
        cancelled ||
        lifecycleGenerationRef.current !== generation ||
        activeRef.current?.host !== host
      ) {
        await host.dispose();
        return;
      }
      if (hostState.status === 'failed') {
        instanceEpochOwner?.end(instanceEpoch);
        setState({ status: 'failed', instanceEpoch, error: hostState.error });
        return;
      }
      setState({ status: 'mounted', instanceEpoch });
      const latestRuntimeInputFactory = createRuntimeInputRef.current;
      if (latestRuntimeInputFactory !== mountedRuntimeInputFactory) {
        const latestPlan = latestRuntimeInputFactory(instanceEpoch).plan;
        setState({ status: 'updating', instanceEpoch });
        const updated = await host.update(latestPlan);
        if (
          !cancelled &&
          lifecycleGenerationRef.current === generation &&
          activeRef.current?.host === host
        ) {
          activeRef.current.mountedPlan = latestPlan;
          activeRef.current.mountedRuntimeInputFactory =
            latestRuntimeInputFactory;
          setState(
            updated.status === 'failed'
              ? { status: 'failed', instanceEpoch, error: updated.error }
              : { status: 'mounted', instanceEpoch }
          );
        }
      }
    };
    void mount();

    return () => {
      cancelled = true;
      const active = activeRef.current;
      if (!active || active.identity !== identity) return;
      activeRef.current = null;
      instanceEpochOwner?.end(active.instanceEpoch);
      setState({ status: 'disposing', instanceEpoch: active.instanceEpoch });
      disposalRef.current = active.host.dispose();
    };
  }, [hostFactory, identity, instanceEpochOwner, root]);

  useEffect(() => {
    const active = activeRef.current;
    if (!active || active.identity !== identity) return;
    if (active.mountedRuntimeInputFactory === createRuntimeInput) return;
    const runtimeInput = createRuntimeInput(active.instanceEpoch);
    active.mountedPlan = runtimeInput.plan;
    active.mountedRuntimeInputFactory = createRuntimeInput;
    let cancelled = false;
    setState({ status: 'updating', instanceEpoch: active.instanceEpoch });
    void active.host.update(runtimeInput.plan).then((hostState) => {
      if (cancelled || activeRef.current !== active) return;
      setState(
        hostState.status === 'failed'
          ? {
              status: 'failed',
              instanceEpoch: active.instanceEpoch,
              error: hostState.error
            }
          : { status: 'mounted', instanceEpoch: active.instanceEpoch }
      );
    });
    return () => {
      cancelled = true;
    };
  }, [createRuntimeInput, identity]);

  return state;
}

function nativeInstanceIdentity(
  identity: FrontstageNativeInstanceMountIntent['identityInput']
): string {
  return JSON.stringify({
    sourceSha256: identity.sourceSha256,
    runtimeFingerprint: identity.runtimeFingerprint,
    dependencyLockIdentity: identity.dependencyLockIdentity
  });
}

function createFrontstageNativeBlockInstanceHost({
  prepared,
  readRuntimeInput
}: {
  prepared: FrontstageNativePreparedRuntime;
  readRuntimeInput(): FrontstageNativeBlockInstanceRuntimeInput;
}): NativeTrustedBlockHost {
  const adapter = createFrontstageNativeTrustedBlockReactAdapter({
    resolveComponent: () =>
      prepared.component as FrontstageNativeTrustedBlockReactComponent,
    resolveBlockContext: () => readRuntimeInput().context,
    resolveProviderScope: () => readRuntimeInput().providerScope
  });
  return createNativeTrustedBlockHost({ adapter });
}
