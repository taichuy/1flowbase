import {
  getDefaultApiBaseUrl,
  listConsoleFrontendBlocks,
  type ApiBaseUrlLocation,
  type ConsoleFrontendBlockCatalogEntry
} from '@1flowbase/api-client';

import {
  fetchExternalNpmPack,
  mergeExternalNpmModules,
  type ExternalNpmModule,
  type ExternalNpmPackState
} from './external-npm';

export type FrontstageBlockCatalogEntry = Omit<
  ConsoleFrontendBlockCatalogEntry,
  'code_modules'
> & {
  code_modules: Array<
    ConsoleFrontendBlockCatalogEntry['code_modules'][number] | ExternalNpmModule
  >;
};

export interface FrontstageBlockCatalogSnapshot {
  entries: FrontstageBlockCatalogEntry[];
  externalNpm: ExternalNpmPackState;
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
  const [entries, externalNpm] = await Promise.all([
    listConsoleFrontendBlocks(getFrontstageBlockCatalogApiBaseUrl()),
    fetchExternalNpmPack()
  ]);
  return {
    entries: mergeExternalNpmModules(entries, externalNpm.modules),
    externalNpm: externalNpm.state
  };
}
