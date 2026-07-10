import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import {
  fetchFrontstageBlockCatalog,
  frontstageBlockCatalogQueryKey,
  frontstageBlockCatalogQueryKeyPrefix
} from '../api/block-catalog';
import {
  normalizeFrontstageBlockCatalog,
  type FrontstageBlockCatalogDiagnostic,
  type NormalizedFrontstageBlockCatalogEntry
} from '../lib/block-catalog';

const emptyCatalog = {
  items: [] as NormalizedFrontstageBlockCatalogEntry[],
  diagnostics: [] as FrontstageBlockCatalogDiagnostic[]
};

function toError(error: unknown): Error {
  return error instanceof Error
    ? error
    : new Error('frontstage block catalog request failed');
}

export function useFrontstageBlockCatalog({
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
  const hasCatalogReadContext = Boolean(
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
      queryKey: frontstageBlockCatalogQueryKeyPrefix
    });
  }, [queryClient, sessionStatus]);

  const blockCatalogQuery = useQuery({
    queryKey: frontstageBlockCatalogQueryKey({
      workspaceId: workspaceId ?? 'missing-workspace',
      actorId: actor?.id ?? 'missing-actor',
      permissionFingerprint: permissionFingerprint ?? 'missing-permissions'
    }),
    queryFn: fetchFrontstageBlockCatalog,
    select: normalizeFrontstageBlockCatalog,
    enabled: hasCatalogReadContext
  });

  const catalog = hasCatalogReadContext
    ? (blockCatalogQuery.data ?? emptyCatalog)
    : emptyCatalog;

  return {
    items: catalog.items,
    diagnostics: catalog.diagnostics,
    loading: blockCatalogQuery.isLoading,
    error: blockCatalogQuery.error ? toError(blockCatalogQuery.error) : null,
    refetch: blockCatalogQuery.refetch,
    status: blockCatalogQuery.status,
    fetchStatus: blockCatalogQuery.fetchStatus,
    isLoading: blockCatalogQuery.isLoading,
    isFetching: blockCatalogQuery.isFetching,
    isRefetching: blockCatalogQuery.isRefetching,
    isError: blockCatalogQuery.isError,
    isSuccess: blockCatalogQuery.isSuccess,
    dataUpdatedAt: blockCatalogQuery.dataUpdatedAt
  };
}
