import { useQuery } from '@tanstack/react-query';

import { useAuthStore } from '../../../state/auth-store';
import {
  fetchFrontstageComponentCapabilities,
  type FrontstageComponentCapabilityPage,
  type FrontstageComponentCapabilityQuery
} from '../api/component-capabilities';

const emptyPage: FrontstageComponentCapabilityPage = {
  items: [],
  total: 0,
  offset: 0,
  limit: 20,
  has_more: false,
  next_offset: null,
  module_sources: []
};

export function useFrontstageComponentCapabilities(
  workspaceId: string | null,
  request: FrontstageComponentCapabilityQuery,
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
    queryKey: ['frontstage', 'component-capabilities', workspaceId, request],
    queryFn: () =>
      fetchFrontstageComponentCapabilities(workspaceId as string, request),
    enabled
  });
  return {
    data: enabled ? (query.data ?? emptyPage) : emptyPage,
    loading: query.isLoading,
    error: query.error instanceof Error ? query.error : null,
    refetch: query.refetch
  };
}
