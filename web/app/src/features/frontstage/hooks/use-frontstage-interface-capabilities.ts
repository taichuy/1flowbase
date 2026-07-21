import { useQuery } from '@tanstack/react-query';

import { useAuthStore } from '../../../state/auth-store';
import { fetchFrontstageInterfaceCapabilities } from '../api/interface-capabilities';

export function useFrontstageInterfaceCapabilities(workspaceId: string | null) {
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  const actor = useAuthStore((state) => state.actor);
  const enabled = Boolean(
    sessionStatus === 'authenticated' &&
      workspaceId &&
      actor?.current_workspace_id === workspaceId
  );
  const query = useQuery({
    queryKey: ['frontstage', 'interface-capabilities', workspaceId],
    queryFn: () => fetchFrontstageInterfaceCapabilities(workspaceId as string),
    enabled
  });
  return {
    data: enabled ? (query.data ?? []) : [],
    loading: query.isLoading,
    error: query.error instanceof Error ? query.error : null,
    refetch: query.refetch
  };
}
