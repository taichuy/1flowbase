import {
  getDefaultApiBaseUrl,
  listConsoleFrontendBlockTypeDeclarations,
  listConsoleFrontendBlocks,
  type ApiBaseUrlLocation,
  type ConsoleFrontendBlockCatalogEntry,
  type ConsoleFrontendBlockTypeDeclaration,
  type FrontendUiRegistrationTarget
} from '@1flowbase/api-client';

export type FrontstageBlockCatalogEntry = ConsoleFrontendBlockCatalogEntry;
export type FrontstageBlockTypeDeclaration =
  ConsoleFrontendBlockTypeDeclaration;

export interface FrontstageBlockCatalogSnapshot {
  entries: FrontstageBlockCatalogEntry[];
}

export const frontstageBlockCatalogQueryKeyPrefix = [
  'frontstage',
  'block-catalog'
] as const;

export const frontstageBlockTypeDeclarationsQueryKeyPrefix = [
  'frontstage',
  'block-type-declarations'
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

export function frontstageBlockTypeDeclarationsQueryKey({
  workspaceId,
  actorId,
  permissionFingerprint
}: {
  workspaceId: string;
  actorId: string;
  permissionFingerprint: string;
}) {
  return [
    ...frontstageBlockTypeDeclarationsQueryKeyPrefix,
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

export function fetchFrontstageBlockTypeDeclarations(): Promise<
  FrontstageBlockTypeDeclaration[]
> {
  return listConsoleFrontendBlockTypeDeclarations(
    getFrontstageBlockCatalogApiBaseUrl()
  );
}
