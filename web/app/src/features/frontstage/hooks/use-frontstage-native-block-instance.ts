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
  runtimeInput: FrontstageNativeBlockInstanceRuntimeInput;
  hostFactory?: FrontstageNativeBlockInstanceHostFactory;
}

interface ActiveNativeBlockInstance {
  identity: string;
  host: NativeTrustedBlockHost;
  mountedPlan: NativeTrustedBlockPreparePlan;
}

export function useFrontstageNativeBlockInstance({
  root,
  mountIntent,
  prepared,
  runtimeInput,
  hostFactory = createFrontstageNativeBlockInstanceHost
}: UseFrontstageNativeBlockInstanceInput): FrontstageNativeBlockInstanceState {
  const [state, setState] = useState<FrontstageNativeBlockInstanceState>({
    status: 'unmounted'
  });
  const activeRef = useRef<ActiveNativeBlockInstance | null>(null);
  const runtimeInputRef = useRef(runtimeInput);
  runtimeInputRef.current = runtimeInput;
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
      const host = hostFactory({
        prepared,
        readRuntimeInput: () => runtimeInputRef.current
      });
      const plan = runtimeInputRef.current.plan;
      activeRef.current = { identity, host, mountedPlan: plan };
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
        setState({ status: 'failed', error: hostState.error });
        return;
      }
      setState({ status: 'mounted' });
      const latestPlan = runtimeInputRef.current.plan;
      if (latestPlan !== plan) {
        setState({ status: 'updating' });
        const updated = await host.update(latestPlan);
        if (
          !cancelled &&
          lifecycleGenerationRef.current === generation &&
          activeRef.current?.host === host
        ) {
          activeRef.current.mountedPlan = latestPlan;
          setState(
            updated.status === 'failed'
              ? { status: 'failed', error: updated.error }
              : { status: 'mounted' }
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
      setState({ status: 'disposing' });
      disposalRef.current = active.host.dispose();
    };
  }, [hostFactory, identity, root]);

  useEffect(() => {
    const active = activeRef.current;
    if (!active || active.identity !== identity) return;
    if (active.mountedPlan === runtimeInput.plan) return;
    active.mountedPlan = runtimeInput.plan;
    let cancelled = false;
    setState({ status: 'updating' });
    void active.host.update(runtimeInput.plan).then((hostState) => {
      if (cancelled || activeRef.current !== active) return;
      setState(
        hostState.status === 'failed'
          ? { status: 'failed', error: hostState.error }
          : { status: 'mounted' }
      );
    });
    return () => {
      cancelled = true;
    };
  }, [identity, runtimeInput]);

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
