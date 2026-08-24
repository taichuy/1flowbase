import {
  getDefaultApiBaseUrl,
  listConsoleFrontendBlocks,
  type ApiBaseUrlLocation,
  type ConsoleFrontendBlockCatalogEntry
} from '@1flowbase/api-client';

export type FrontstageBlockCatalogEntry = ConsoleFrontendBlockCatalogEntry;

export interface FrontstageBlockCatalogSnapshot {
  entries: FrontstageBlockCatalogEntry[];
}

export const frontstageBlockCatalogQueryKeyPrefix = [
  'frontstage',
  'block-catalog'
] as const;

export function frontstageBlockCatalogQueryKey({
  workspaceId,
  actorId,
  permissionFingerprint
}: {
  workspaceId: string;
  actorId: string;
  permissionFingerprint: string;
}) {
  return [
    ...frontstageBlockCatalogQueryKeyPrefix,
    workspaceId,
    actorId,
    permissionFingerprint
  ] as const;
}

export function getFrontstageBlockCatalogApiBaseUrl(
  locationLike: ApiBaseUrlLocation | undefined = typeof window !== 'undefined'
    ? window.location
    : undefined
): string {
  return (
    import.meta.env.VITE_API_BASE_URL ?? getDefaultApiBaseUrl(locationLike)
  );
}

export async function fetchFrontstageBlockCatalog(): Promise<FrontstageBlockCatalogSnapshot> {
  return {
    entries: await listConsoleFrontendBlocks(
      getFrontstageBlockCatalogApiBaseUrl()
    )
  };
}
