import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState
} from 'react';

import type { FrontstageNativeInstanceMountIntent } from '../lib/page-canvas/native-runtime-preparation';
import {
  recordFrontstageRuntimeObservation,
  type FrontstageRuntimeObservationCacheTier,
  type FrontstageRuntimeObservationContext
} from '../lib/page-canvas/runtime-observation';
import type { FrontstageSignalRuntimeCoordinator } from '../lib/page-canvas/signal-runtime';

export interface UseFrontstageNativeBlockInstanceInput {
  blockId: string;
  signalCoordinator?: FrontstageSignalRuntimeCoordinator | null;
  observationContext?: FrontstageRuntimeObservationContext;
  cacheTier?: FrontstageRuntimeObservationCacheTier;
  preparationGeneration?: number;
}

export interface FrontstageNativeBlockInstance {
  instanceEpoch: string;
  isCurrentInstance(): boolean;
}

let nextStandaloneInstanceEpoch = 0;

/**
 * Registers the epoch owned by one declaratively mounted Portal instance.
 * React owns component/portal lifecycle; this hook only mirrors that lifecycle
 * into the page-scoped Signal coordinator.
 */
export function useFrontstageNativeBlockInstance({
  blockId,
  signalCoordinator,
  observationContext,
  cacheTier,
  preparationGeneration
}: UseFrontstageNativeBlockInstanceInput): FrontstageNativeBlockInstance {
  const [instanceEpoch] = useState(
    () => `${blockId}:portal-${++nextStandaloneInstanceEpoch}`
  );
  const currentInstanceEpochRef = useRef<string | null>(null);
  const observationRef = useRef({
    context: observationContext,
    cacheTier,
    generation: preparationGeneration
  });
  observationRef.current = {
    context: observationContext,
    cacheTier,
    generation: preparationGeneration
  };

  useLayoutEffect(() => {
    const registeredEpoch =
      signalCoordinator?.beginInstance(blockId, instanceEpoch) ?? instanceEpoch;
    currentInstanceEpochRef.current = registeredEpoch;
    const observation = observationRef.current;
    if (observation.context && observation.cacheTier) {
      recordFrontstageRuntimeObservation({
        ...observation.context,
        stage: 'shadow_attach',
        runtimeKind: 'native',
        cacheTier: observation.cacheTier,
        generation: observation.generation,
        instanceEpoch: registeredEpoch
      });
    }

    return () => {
      if (currentInstanceEpochRef.current === registeredEpoch) {
        currentInstanceEpochRef.current = null;
      }
      signalCoordinator?.endInstance(blockId, registeredEpoch);
    };
  }, [blockId, instanceEpoch, signalCoordinator]);

  useEffect(() => {
    const observation = observationRef.current;
    if (!observation.context || !observation.cacheTier) return;
    for (const stage of ['react_mount', 'present'] as const) {
      recordFrontstageRuntimeObservation({
        ...observation.context,
        stage,
        runtimeKind: 'native',
        cacheTier: observation.cacheTier,
        generation: observation.generation,
        instanceEpoch
      });
    }
  }, [instanceEpoch]);

  const isCurrentInstance = useCallback(
    () => currentInstanceEpochRef.current === instanceEpoch,
    [instanceEpoch]
  );

  return { instanceEpoch, isCurrentInstance };
}

export function frontstageNativeInstanceRenderKey(
  mountIntent: FrontstageNativeInstanceMountIntent
): string {
  const { identityInput } = mountIntent;
  return JSON.stringify({
    sourceSha256: identityInput.sourceSha256,
    runtimeFingerprint: identityInput.runtimeFingerprint,
    dependencyLockIdentity: identityInput.dependencyLockIdentity
  });
}
