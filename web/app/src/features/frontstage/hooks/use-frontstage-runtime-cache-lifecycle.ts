import { useQueryClient } from '@tanstack/react-query';
import { useLayoutEffect, useRef } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import { clearFrontstageRuntimeSessionCache } from './use-frontstage-page-canvas-runtime-sessions';
import { resetFrontstageRuntimeObservations } from '../lib/page-canvas/runtime-observation';

export function useFrontstageRuntimeCacheLifecycle(): void {
  const queryClient = useQueryClient();
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  const actor = useAuthStore((state) => state.actor);
  const lifecycleIdentity = actor
    ? `actor:${actor.id}:workspace:${actor.current_workspace_id}`
    : sessionStatus;
  const previousIdentityRef = useRef<string | null>(null);

  useLayoutEffect(() => {
    if (previousIdentityRef.current === lifecycleIdentity) {
      return;
    }
    previousIdentityRef.current = lifecycleIdentity;
    clearFrontstageRuntimeSessionCache();
    queryClient.removeQueries({
      predicate: (query) => query.queryKey[0] === 'frontstage'
    });
    resetFrontstageRuntimeObservations();
  }, [lifecycleIdentity, queryClient]);
}
