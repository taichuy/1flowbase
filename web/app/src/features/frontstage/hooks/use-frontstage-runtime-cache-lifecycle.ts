import { useQueryClient } from '@tanstack/react-query';
import { useLayoutEffect } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../state/navigation-query-scope';
import { resetFrontstageRuntimeObservations } from '../lib/page-canvas/runtime-observation';
import { frontstageNativeReactArtifactCache } from '../lib/runtime-cache/default-cache';
import type { FrontstageNativeReactArtifactCache } from '../lib/runtime-cache/native-react-artifact-cache';

export interface FrontstageRuntimeCacheLifecycleOptions {
  nativeReactArtifactCache?: Pick<
    FrontstageNativeReactArtifactCache,
    'deleteActor' | 'pruneWorkspace'
  >;
}

export function FrontstageRuntimeCacheLifecycle() {
  useFrontstageRuntimeCacheLifecycle();
  return null;
}

export function useFrontstageRuntimeCacheLifecycle(
  options: FrontstageRuntimeCacheLifecycleOptions = {}
): void {
  const queryClient = useQueryClient();
  const nativeReactArtifactCache =
    options.nativeReactArtifactCache ?? frontstageNativeReactArtifactCache;

  useLayoutEffect(() => {
    let previousIdentity: string | null = null;
    let previousActorId: string | null = null;
    const synchronize = () => {
      const state = useAuthStore.getState();
      const actor = state.actor;
      const scope = selectNavigationQueryScope(state);
      const identity =
        state.sessionStatus === 'authenticated' && actor
          ? scope
          : state.sessionStatus;
      if (previousIdentity === identity) return;
      const hadPreviousIdentity = previousIdentity !== null;
      previousIdentity = identity;
      const currentActorId = actor?.id ?? null;
      if (hadPreviousIdentity) {
        queryClient.removeQueries({
          predicate: (query) =>
            query.queryKey[0] === 'frontstage' &&
            !(
              // A store subscriber may already have prefetched the NEW scope.
              // Never discard that in-flight request or its successful result.
              (
                state.sessionStatus === 'authenticated' &&
                actor &&
                query.queryKey[1] === actor.current_workspace_id &&
                query.queryKey[2] === 'page-tree' &&
                query.queryKey[3] === scope
              )
            )
        });
      }
      resetFrontstageRuntimeObservations();
      if (previousActorId && previousActorId !== currentActorId) {
        void nativeReactArtifactCache
          .deleteActor(previousActorId)
          .catch(() => undefined);
      }
      previousActorId = currentActorId;
      if (actor) {
        void nativeReactArtifactCache
          .pruneWorkspace({
            actorId: actor.id,
            workspaceId: actor.current_workspace_id
          })
          .catch(() => undefined);
      }
    };
    // Synchronous store observation handles logout even when the authenticated
    // runtime unmounts in the same React commit.
    const unsubscribe = useAuthStore.subscribe(synchronize);
    synchronize();
    return unsubscribe;
  }, [nativeReactArtifactCache, queryClient]);
}
