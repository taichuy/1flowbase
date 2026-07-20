import { useQuery } from '@tanstack/react-query';

import { useAuthStore } from '../../../state/auth-store';
import { fetchFrontstageCallableInterfaces } from '../api/callable-interfaces';

export function useFrontstageCallableInterfaces(workspaceId: string | null) {
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  const actor = useAuthStore((state) => state.actor);
  const enabled = Boolean(
    sessionStatus === 'authenticated' &&
      workspaceId &&
      actor?.current_workspace_id === workspaceId
  );
  const query = useQuery({
    queryKey: ['frontstage', 'callable-interfaces', workspaceId],
    queryFn: () => fetchFrontstageCallableInterfaces(workspaceId as string),
    enabled
  });
  return {
    data: enabled ? (query.data ?? []) : [],
    loading: query.isLoading,
    error: query.error instanceof Error ? query.error : null,
    refetch: query.refetch
  };
}
