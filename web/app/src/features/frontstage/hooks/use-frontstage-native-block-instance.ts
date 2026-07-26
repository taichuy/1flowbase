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
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  createFrontstageNativeTrustedBlockReactAdapter,
  type FrontstageNativeTrustedBlockReactComponent
} from '../lib/native-trusted-block-react-adapter';
import type {
  FrontstageNativeInstanceMountIntent,
  FrontstageNativePreparedRuntime
} from '../lib/page-canvas/native-runtime-preparation';
import {
  recordFrontstageRuntimeObservation,
  type FrontstageNativeRuntimeObservationStage,
  type FrontstageRuntimeObservationContext
} from '../lib/page-canvas/runtime-observation';

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
  retry(): void;
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
  onRuntimeError(error: BlockProtocolError): void;
}) => NativeTrustedBlockHost;

export interface UseFrontstageNativeBlockInstanceInput {
  root: Element | null;
  mountIntent: FrontstageNativeInstanceMountIntent | null;
  prepared: FrontstageNativePreparedRuntime | null;
  createRuntimeInput(
    instanceEpoch: string
  ): FrontstageNativeBlockInstanceRuntimeInput;
  runtimeInputRevision: unknown;
  instanceEpochOwner?: FrontstageNativeInstanceEpochOwner;
  hostFactory?: FrontstageNativeBlockInstanceHostFactory;
  observationContext?: FrontstageRuntimeObservationContext;
  preparationGeneration?: number;
}

interface ActiveNativeBlockInstance {
  identity: string;
  instanceEpoch: string;
  host: NativeTrustedBlockHost;
  mountedPlan: NativeTrustedBlockPreparePlan;
  mountedRuntimeInputRevision: unknown;
  epochEnded: boolean;
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
  runtimeInputRevision,
  instanceEpochOwner,
  hostFactory = createFrontstageNativeBlockInstanceHost,
  observationContext,
  preparationGeneration
}: UseFrontstageNativeBlockInstanceInput): FrontstageNativeBlockInstanceState {
  const [state, setState] = useState<
    Omit<FrontstageNativeBlockInstanceState, 'retry'>
  >({
    status: 'unmounted'
  });
  const [retryGeneration, setRetryGeneration] = useState(0);
  const activeRef = useRef<ActiveNativeBlockInstance | null>(null);
  const createRuntimeInputRef = useRef(createRuntimeInput);
  createRuntimeInputRef.current = createRuntimeInput;
  const runtimeInputRevisionRef = useRef(runtimeInputRevision);
  runtimeInputRevisionRef.current = runtimeInputRevision;
  const observationContextRef = useRef(observationContext);
  observationContextRef.current = observationContext;
  const observationMetadataRef = useRef({
    cacheTier: prepared?.artifactCacheTier,
    generation: preparationGeneration
  });
  observationMetadataRef.current = {
    cacheTier: prepared?.artifactCacheTier,
    generation: preparationGeneration
  };
  const lifecycleGenerationRef = useRef(0);
  const disposalRef = useRef<Promise<unknown>>(Promise.resolve());
  const identity = useMemo(
    () =>
      mountIntent ? nativeInstanceIdentity(mountIntent.identityInput) : null,
    [mountIntent]
  );
  const retry = useCallback(() => {
    setState({ status: 'unmounted' });
    setRetryGeneration((current) => current + 1);
  }, []);
  const observe = useCallback(
    (stage: FrontstageNativeRuntimeObservationStage, instanceEpoch: string) => {
      const currentObservationContext = observationContextRef.current;
      const metadata = observationMetadataRef.current;
      if (!currentObservationContext || !metadata.cacheTier) return;
      recordFrontstageRuntimeObservation({
        ...currentObservationContext,
        stage,
        runtimeKind: 'native',
        cacheTier: metadata.cacheTier,
        generation: metadata.generation,
        instanceEpoch
      });
    },
    []
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
      let host: NativeTrustedBlockHost;
      let runtimeError: BlockProtocolError | undefined;
      const onRuntimeError = (error: BlockProtocolError) => {
        if (
          cancelled ||
          lifecycleGenerationRef.current !== generation ||
          activeRef.current?.host !== host
        )
          return;
        runtimeError = error;
        setState({ status: 'failed', instanceEpoch, error });
      };
      host = hostFactory({
        prepared,
        readRuntimeInput: () => createRuntimeInputRef.current(instanceEpoch),
        onRuntimeError
      });
      const mountedRuntimeInputRevision = runtimeInputRevisionRef.current;
      const plan = createRuntimeInputRef.current(instanceEpoch).plan;
      activeRef.current = {
        identity,
        instanceEpoch,
        host,
        mountedPlan: plan,
        mountedRuntimeInputRevision,
        epochEnded: false
      };
      observe('shadow_attach', instanceEpoch);
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
        endInstanceEpoch(activeRef.current, instanceEpochOwner);
        setState({ status: 'failed', instanceEpoch, error: hostState.error });
        return;
      }
      if (runtimeError) return;
      observe('react_mount', instanceEpoch);
      setState({ status: 'mounted', instanceEpoch });
      observe('present', instanceEpoch);
      const latestRuntimeInputRevision = runtimeInputRevisionRef.current;
      if (latestRuntimeInputRevision !== mountedRuntimeInputRevision) {
        const latestPlan = createRuntimeInputRef.current(instanceEpoch).plan;
        setState({ status: 'updating', instanceEpoch });
        const updated = await host.update(latestPlan);
        if (
          !cancelled &&
          lifecycleGenerationRef.current === generation &&
          activeRef.current?.host === host
        ) {
          activeRef.current.mountedPlan = latestPlan;
          activeRef.current.mountedRuntimeInputRevision =
            latestRuntimeInputRevision;
          if (updated.status === 'failed') {
            setState({ status: 'failed', instanceEpoch, error: updated.error });
          } else {
            setState({ status: 'mounted', instanceEpoch });
            observe('present', instanceEpoch);
          }
        }
      }
    };
    void mount();

    return () => {
      cancelled = true;
      const active = activeRef.current;
      if (!active || active.identity !== identity) return;
      activeRef.current = null;
      endInstanceEpoch(active, instanceEpochOwner);
      setState({ status: 'disposing', instanceEpoch: active.instanceEpoch });
      disposalRef.current = active.host.dispose();
    };
  }, [
    hostFactory,
    identity,
    instanceEpochOwner,
    observe,
    retryGeneration,
    root
  ]);

  useEffect(() => {
    const active = activeRef.current;
    if (!active || active.identity !== identity) return;
    if (active.mountedRuntimeInputRevision === runtimeInputRevision) return;
    const runtimeInput = createRuntimeInputRef.current(active.instanceEpoch);
    active.mountedPlan = runtimeInput.plan;
    active.mountedRuntimeInputRevision = runtimeInputRevision;
    let cancelled = false;
    setState({ status: 'updating', instanceEpoch: active.instanceEpoch });
    void active.host.update(runtimeInput.plan).then((hostState) => {
      if (cancelled || activeRef.current !== active) return;
      if (hostState.status === 'failed') {
        setState({
          status: 'failed',
          instanceEpoch: active.instanceEpoch,
          error: hostState.error
        });
      } else {
        setState({ status: 'mounted', instanceEpoch: active.instanceEpoch });
        observe('present', active.instanceEpoch);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [identity, observe, runtimeInputRevision]);

  return { ...state, retry };
}

function endInstanceEpoch(
  active: ActiveNativeBlockInstance | null,
  owner: FrontstageNativeInstanceEpochOwner | undefined
): void {
  if (!active || active.epochEnded) return;
  active.epochEnded = true;
  owner?.end(active.instanceEpoch);
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
  readRuntimeInput,
  onRuntimeError
}: {
  prepared: FrontstageNativePreparedRuntime;
  readRuntimeInput(): FrontstageNativeBlockInstanceRuntimeInput;
  onRuntimeError(error: BlockProtocolError): void;
}): NativeTrustedBlockHost {
  const adapter = createFrontstageNativeTrustedBlockReactAdapter({
    resolveComponent: () =>
      prepared.component as FrontstageNativeTrustedBlockReactComponent,
    resolveBlockContext: () => readRuntimeInput().context,
    resolveProviderScope: () => readRuntimeInput().providerScope,
    onRuntimeError
  });
  return createNativeTrustedBlockHost({ adapter });
}
