import { useQueryClient } from '@tanstack/react-query';
import { useLayoutEffect, useRef } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import { resetFrontstageRuntimeObservations } from '../lib/page-canvas/runtime-observation';
import {
  frontstageNativeReactArtifactCache,
  type FrontstageNativeReactArtifactCache
} from '../lib/runtime-cache';
import { getNativeReactRuntimeFingerprint } from '../../../shared/code-block/native-react-compiler-browser';

export interface FrontstageRuntimeCacheLifecycleOptions {
  nativeReactArtifactCache?: Pick<
    FrontstageNativeReactArtifactCache,
    'deleteActor' | 'pruneWorkspace'
  >;
  nativeReactRuntimeFingerprint?: string;
}

export function useFrontstageRuntimeCacheLifecycle(
  options: FrontstageRuntimeCacheLifecycleOptions = {}
): void {
  const queryClient = useQueryClient();
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  const actor = useAuthStore((state) => state.actor);
  const lifecycleIdentity = actor
    ? `actor:${actor.id}:workspace:${actor.current_workspace_id}`
    : sessionStatus;
  const previousIdentityRef = useRef<string | null>(null);
  const previousActorIdRef = useRef<string | null>(null);
  const nativeReactArtifactCache =
    options.nativeReactArtifactCache ?? frontstageNativeReactArtifactCache;
  const nativeReactRuntimeFingerprint =
    options.nativeReactRuntimeFingerprint ?? getNativeReactRuntimeFingerprint();

  useLayoutEffect(() => {
    if (previousIdentityRef.current === lifecycleIdentity) {
      return;
    }
    previousIdentityRef.current = lifecycleIdentity;
    const previousActorId = previousActorIdRef.current;
    const currentActorId = actor?.id ?? null;
    previousActorIdRef.current = currentActorId;
    queryClient.removeQueries({
      predicate: (query) => query.queryKey[0] === 'frontstage'
    });
    resetFrontstageRuntimeObservations();
    if (previousActorId && previousActorId !== currentActorId) {
      void nativeReactArtifactCache
        .deleteActor(previousActorId)
        .catch(() => undefined);
    }
    if (actor) {
      void nativeReactArtifactCache
        .pruneWorkspace({
          actorId: actor.id,
          workspaceId: actor.current_workspace_id,
          runtimeFingerprint: nativeReactRuntimeFingerprint
        })
        .catch(() => undefined);
    }
  }, [
    actor,
    lifecycleIdentity,
    nativeReactArtifactCache,
    nativeReactRuntimeFingerprint,
    queryClient
  ]);
}
