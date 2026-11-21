import { useQueryClient } from '@tanstack/react-query';
import { useLayoutEffect } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../state/navigation-query-scope';
import { createFrontstageBlockCatalogPermissionFingerprint } from '../api/block-catalog-scope';
import { resetFrontstageRuntimeObservations } from '../lib/page-canvas/runtime-observation';
import {
  frontstageNativeBlockSourceCache,
  frontstageNativeReactArtifactCache
} from '../lib/runtime-cache/default-cache';
import type { FrontstageNativeBlockSourceCache } from '../lib/runtime-cache/native-block-source-cache';
import type { FrontstageNativeReactArtifactCache } from '../lib/runtime-cache/native-react-artifact-cache';

export interface FrontstageRuntimeCacheLifecycleOptions {
  nativeReactArtifactCache?: Pick<
    FrontstageNativeReactArtifactCache,
    'deleteActor' | 'pruneWorkspace'
  >;
  nativeBlockSourceCache?: Pick<
    FrontstageNativeBlockSourceCache,
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
  const nativeBlockSourceCache =
    options.nativeBlockSourceCache ?? frontstageNativeBlockSourceCache;

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
        const currentPermissionFingerprint = state.me
          ? createFrontstageBlockCatalogPermissionFingerprint(state.me)
          : null;
        const currentSortedPermissions = state.me
          ? state.me.permissions.slice().sort().join(',')
          : null;
        queryClient.removeQueries({
          predicate: (query) =>
            query.queryKey[0] === 'frontstage' &&
            !(
              // A store subscriber may already have prefetched the NEW scope.
              // Never discard that in-flight request or its successful result.
              (
                state.sessionStatus === 'authenticated' &&
                actor &&
                ((query.queryKey[1] === actor.current_workspace_id &&
                  query.queryKey[2] === 'page-tree' &&
                  query.queryKey[3] === scope) ||
                  (currentPermissionFingerprint !== null &&
                    query.queryKey[1] === 'block-catalog' &&
                    query.queryKey[2] === actor.current_workspace_id &&
                    query.queryKey[3] === actor.id &&
                    query.queryKey[4] === currentPermissionFingerprint) ||
                  (currentSortedPermissions !== null &&
                    query.queryKey[1] === actor.current_workspace_id &&
                    query.queryKey[2] === 'runtime-i18n' &&
                    query.queryKey[3] === actor.id &&
                    query.queryKey[4] === currentSortedPermissions))
              )
            )
        });
      }
      resetFrontstageRuntimeObservations();
      if (previousActorId && previousActorId !== currentActorId) {
        void nativeReactArtifactCache
          .deleteActor(previousActorId)
          .catch(() => undefined);
        void nativeBlockSourceCache
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
        void nativeBlockSourceCache
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
  }, [nativeBlockSourceCache, nativeReactArtifactCache, queryClient]);
}
