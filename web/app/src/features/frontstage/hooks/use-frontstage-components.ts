import { useQuery } from '@tanstack/react-query';

import { useAuthStore } from '../../../state/auth-store';
import {
  fetchFrontstageComponents,
  type FrontstageComponentPage,
  type FrontstageComponentQuery
} from '../api/components';

const emptyPage: FrontstageComponentPage = {
  items: [],
  total: 0,
  offset: 0,
  limit: 20,
  has_more: false,
  next_offset: null
};

export function useFrontstageComponents(
  workspaceId: string | null,
  request: FrontstageComponentQuery,
  active = true
) {
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  const actor = useAuthStore((state) => state.actor);
  const enabled = Boolean(
    active &&
    sessionStatus === 'authenticated' &&
    workspaceId &&
    actor?.current_workspace_id === workspaceId
  );
  const query = useQuery({
    queryKey: ['frontstage', 'components', workspaceId, request],
    queryFn: () => fetchFrontstageComponents(workspaceId as string, request),
    enabled
  });
  return {
    data: enabled ? (query.data ?? emptyPage) : emptyPage,
    loading: query.isLoading,
    error: query.error instanceof Error ? query.error : null,
    refetch: query.refetch
  };
}
