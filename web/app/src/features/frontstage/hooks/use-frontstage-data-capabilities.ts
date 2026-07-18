import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import {
  fetchFrontstageDataCapabilities,
  frontstageDataCapabilitiesQueryKey,
  frontstageDataCapabilitiesQueryKeyPrefix,
  type FrontstageDataCapabilities
} from '../api/data-capabilities';

const emptyCapabilities: FrontstageDataCapabilities = {
  queries: [],
  actions: [],
  models: []
};

export function useFrontstageDataCapabilities({
  workspaceId
}: {
  workspaceId: string | null | undefined;
}) {
  const queryClient = useQueryClient();
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const permissionFingerprint = me
    ? `role:${me.effective_display_role}|permissions:${[...me.permissions]
        .sort()
        .join(',')}`
    : null;
  const canRead = Boolean(
    sessionStatus === 'authenticated' &&
      workspaceId &&
      actor &&
      me &&
      permissionFingerprint &&
      actor.current_workspace_id === workspaceId
  );

  useEffect(() => {
    if (sessionStatus !== 'anonymous') {
      return;
    }

    queryClient.removeQueries({
      queryKey: frontstageDataCapabilitiesQueryKeyPrefix
    });
  }, [queryClient, sessionStatus]);

  const query = useQuery({
    queryKey: frontstageDataCapabilitiesQueryKey({
      workspaceId: workspaceId ?? 'missing-workspace',
      actorId: actor?.id ?? 'missing-actor',
      permissionFingerprint: permissionFingerprint ?? 'missing-permissions'
    }),
    queryFn: () => fetchFrontstageDataCapabilities(workspaceId as string),
    enabled: canRead
  });

  return {
    data: canRead ? (query.data ?? emptyCapabilities) : emptyCapabilities,
    loading: query.isLoading,
    error: query.error instanceof Error ? query.error : null,
    refetch: query.refetch
  };
}
