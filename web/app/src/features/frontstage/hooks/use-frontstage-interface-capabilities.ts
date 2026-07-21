import { useQueries, useQuery } from '@tanstack/react-query';

import { useAuthStore } from '../../../state/auth-store';
import {
  fetchFrontstageInterfaceCapability,
  fetchFrontstageInterfaceCapabilities,
  type FrontstageInterfaceCapabilityPage,
  type FrontstageInterfaceCapabilityQuery
} from '../api/interface-capabilities';

const emptyPage: FrontstageInterfaceCapabilityPage = {
  items: [],
  total: 0,
  offset: 0,
  limit: 20,
  has_more: false,
  next_offset: null,
  adapter_ids: [],
  methods: []
};

export function useFrontstageInterfaceCapabilities(
  workspaceId: string | null,
  request: FrontstageInterfaceCapabilityQuery,
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
    queryKey: ['frontstage', 'interface-capabilities', workspaceId, request],
    queryFn: () =>
      fetchFrontstageInterfaceCapabilities(workspaceId as string, request),
    enabled
  });
  return {
    data: enabled ? (query.data ?? emptyPage) : emptyPage,
    loading: query.isLoading,
    error: query.error instanceof Error ? query.error : null,
    refetch: query.refetch
  };
}

export function useFrontstageInterfaceCapabilityDetails(
  workspaceId: string | null,
  interfaceIds: readonly string[],
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
  const queries = useQueries({
    queries: interfaceIds.map((interfaceId) => ({
      queryKey: [
        'frontstage',
        'interface-capabilities',
        workspaceId,
        interfaceId
      ],
      queryFn: () =>
        fetchFrontstageInterfaceCapability(workspaceId as string, interfaceId),
      enabled
    }))
  });
  return {
    data: enabled
      ? queries.flatMap((query) => (query.data ? [query.data] : []))
      : [],
    loading: enabled && queries.some((query) => query.isLoading),
    error: queries.find((query) => query.error instanceof Error)?.error ?? null
  };
}
