import {
  getDefaultApiBaseUrl,
  listConsoleFrontendBlocks,
  type ApiBaseUrlLocation,
  type ConsoleFrontendBlockCatalogEntry
} from '@1flowbase/api-client';

import {
  fetchExternalNpmModules,
  mergeExternalNpmModules
} from './external-npm';

export type FrontstageBlockCatalogEntry = Omit<
  ConsoleFrontendBlockCatalogEntry,
  'code_modules'
> & {
  code_modules: Array<
    Omit<ConsoleFrontendBlockCatalogEntry['code_modules'][number], 'assets'> & {
      assets: Array<
        ConsoleFrontendBlockCatalogEntry['code_modules'][number]['assets'][number] & {
          url?: string;
        }
      >;
    }
  >;
};

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

export async function fetchFrontstageBlockCatalog(): Promise<
  FrontstageBlockCatalogEntry[]
> {
  const [entries, externalModules] = await Promise.all([
    listConsoleFrontendBlocks(getFrontstageBlockCatalogApiBaseUrl()),
    fetchExternalNpmModules()
  ]);
  return mergeExternalNpmModules(entries, externalModules);
}
