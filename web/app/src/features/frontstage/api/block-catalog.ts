import {
  getDefaultApiBaseUrl,
  listConsoleFrontendBlocks,
  type ApiBaseUrlLocation,
  type FrontendUiRegistrationTarget,
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
  permissionFingerprint,
  registrationTarget
}: {
  workspaceId: string;
  actorId: string;
  permissionFingerprint: string;
  registrationTarget?: FrontendUiRegistrationTarget;
}) {
  return [
    ...frontstageBlockCatalogQueryKeyPrefix,
    workspaceId,
    actorId,
    permissionFingerprint,
    ...(registrationTarget ? [{ registration_target: registrationTarget }] : [])
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

export async function fetchFrontstageBlockCatalog(
  registrationTarget?: FrontendUiRegistrationTarget
): Promise<FrontstageBlockCatalogSnapshot> {
  return {
    entries: await listConsoleFrontendBlocks(
      getFrontstageBlockCatalogApiBaseUrl(),
      registrationTarget
        ? { registration_target: registrationTarget }
        : undefined
    )
  };
}
