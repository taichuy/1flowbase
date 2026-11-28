import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo } from 'react';

import { useAuthStore } from '../../../state/auth-store';
import { useConsoleOperationAccess } from '../../../state/console-operation-access';
import {
  fetchFrontstageBlockTypeDeclarations,
  frontstageBlockTypeDeclarationsQueryKey,
  frontstageBlockTypeDeclarationsQueryKeyPrefix,
  type FrontstageBlockTypeDeclaration
} from '../api/block-catalog';
import {
  createFrontstageBlockCatalogPermissionFingerprint,
  frontstageBlockCatalogQueryStaleTime
} from '../api/block-catalog-scope';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import { FRONTSTAGE_NATIVE_REACT_MODULE_DEFINITIONS } from '../lib/native-modules/registry';

const EMPTY_TYPE_DECLARATIONS: readonly FrontstageBlockTypeDeclaration[] = [];

const HOST_NATIVE_REACT_MODULE_SOURCES = new Set(
  FRONTSTAGE_NATIVE_REACT_MODULE_DEFINITIONS.map(
    ({ module_source }) => module_source
  )
);

function hasOnDemandPluginTypeDeclarationCandidates(
  entries: readonly NormalizedFrontstageBlockCatalogEntry[]
): boolean {
  return entries.some(
    (entry) =>
      entry.raw?.disable_reason === null &&
      (entry.raw?.code_modules?.some(
        (module) =>
          module.binding === 'fetched' &&
          !HOST_NATIVE_REACT_MODULE_SOURCES.has(module.source)
      ) ??
        false)
  );
}

export function useFrontstageBlockTypeDeclarations({
  workspaceId,
  open,
  catalogEntries
}: {
  workspaceId: string | null | undefined;
  open: boolean;
  catalogEntries: readonly NormalizedFrontstageBlockCatalogEntry[];
}) {
  const can = useConsoleOperationAccess();
  const queryClient = useQueryClient();
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const permissionFingerprint = me
    ? createFrontstageBlockCatalogPermissionFingerprint(me)
    : null;
  const hasPluginModules = useMemo(
    () => hasOnDemandPluginTypeDeclarationCandidates(catalogEntries),
    [catalogEntries]
  );
  const canReadTypeDeclarations = Boolean(
    open &&
      hasPluginModules &&
      sessionStatus === 'authenticated' &&
      workspaceId &&
      actor &&
      me &&
      permissionFingerprint &&
      actor.current_workspace_id === workspaceId &&
      can('frontend_blocks.type_declarations.view')
  );

  useEffect(() => {
    if (sessionStatus !== 'anonymous') {
      return;
    }

    queryClient.removeQueries({
      queryKey: frontstageBlockTypeDeclarationsQueryKeyPrefix
    });
  }, [queryClient, sessionStatus]);

  const query = useQuery({
    queryKey: frontstageBlockTypeDeclarationsQueryKey({
      workspaceId: workspaceId ?? 'missing-workspace',
      actorId: actor?.id ?? 'missing-actor',
      permissionFingerprint: permissionFingerprint ?? 'missing-permissions'
    }),
    queryFn: () => fetchFrontstageBlockTypeDeclarations(),
    staleTime: frontstageBlockCatalogQueryStaleTime,
    enabled: canReadTypeDeclarations
  });

  return {
    items: canReadTypeDeclarations
      ? (query.data ?? EMPTY_TYPE_DECLARATIONS)
      : EMPTY_TYPE_DECLARATIONS,
    loading: query.isLoading,
    error: query.error ? query.error : null
  };
}
