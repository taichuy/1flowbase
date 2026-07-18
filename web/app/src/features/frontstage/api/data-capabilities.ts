import {
  listFrontstageDataCapabilities,
  type ConsoleFrontstageDataCapabilities
} from '@1flowbase/api-client';

import { getFrontstageApiBaseUrl } from './page-tree';

export type FrontstageDataCapabilities = ConsoleFrontstageDataCapabilities;

export const frontstageDataCapabilitiesQueryKeyPrefix = [
  'frontstage',
  'data-capabilities'
] as const;

export function frontstageDataCapabilitiesQueryKey({
  workspaceId,
  actorId,
  permissionFingerprint
}: {
  workspaceId: string;
  actorId: string;
  permissionFingerprint: string;
}) {
  return [
    ...frontstageDataCapabilitiesQueryKeyPrefix,
    workspaceId,
    actorId,
    permissionFingerprint
  ] as const;
}

export function fetchFrontstageDataCapabilities(
  workspaceId: string
): Promise<FrontstageDataCapabilities> {
  return listFrontstageDataCapabilities(
    workspaceId,
    getFrontstageApiBaseUrl()
  );
}
