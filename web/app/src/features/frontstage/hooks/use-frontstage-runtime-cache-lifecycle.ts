import { useQueryClient } from '@tanstack/react-query';
import { useLayoutEffect, useRef } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import { clearFrontstageRuntimeSessionCache } from './use-frontstage-page-canvas-runtime-sessions';
import { resetFrontstageRuntimeObservations } from '../lib/page-canvas/runtime-observation';
import {
  frontstageCompiledArtifactCache,
  type FrontstageCompiledArtifactCache
} from '../lib/runtime-cache';
import { createCompiledBlockRuntimeFingerprint } from '@1flowbase/page-runtime';
import { getFrontstageRestrictedBlockWorkerUrl } from '../lib/restricted-block-worker-factory';

export interface FrontstageRuntimeCacheLifecycleOptions {
  artifactCache?: Pick<
    FrontstageCompiledArtifactCache,
    'deleteActor' | 'pruneWorkspace'
  >;
  runtimeFingerprint?: string;
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
  const artifactCache = options.artifactCache ?? frontstageCompiledArtifactCache;
  const runtimeFingerprint =
    options.runtimeFingerprint ??
    createCompiledBlockRuntimeFingerprint(
      getFrontstageRestrictedBlockWorkerUrl()
    );

  useLayoutEffect(() => {
    if (previousIdentityRef.current === lifecycleIdentity) {
      return;
    }
    previousIdentityRef.current = lifecycleIdentity;
    const previousActorId = previousActorIdRef.current;
    const currentActorId = actor?.id ?? null;
    previousActorIdRef.current = currentActorId;
    clearFrontstageRuntimeSessionCache();
    queryClient.removeQueries({
      predicate: (query) => query.queryKey[0] === 'frontstage'
    });
    resetFrontstageRuntimeObservations();
    if (previousActorId && previousActorId !== currentActorId) {
      void artifactCache.deleteActor(previousActorId).catch(() => undefined);
    }
    if (actor) {
      void artifactCache
        .pruneWorkspace({
          actorId: actor.id,
          workspaceId: actor.current_workspace_id,
          runtimeFingerprint
        })
        .catch(() => undefined);
    }
  }, [actor, artifactCache, lifecycleIdentity, queryClient, runtimeFingerprint]);
}
