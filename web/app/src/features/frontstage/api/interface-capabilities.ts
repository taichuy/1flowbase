import {
  listFrontstageInterfaceCapabilities,
  type ConsoleFrontstageInterfaceCapability
} from '@1flowbase/api-client';

import { getFrontstageApiBaseUrl } from './page-tree';

export type FrontstageInterfaceCapability =
  ConsoleFrontstageInterfaceCapability;

export function fetchFrontstageInterfaceCapabilities(
  workspaceId: string
): Promise<FrontstageInterfaceCapability[]> {
  return listFrontstageInterfaceCapabilities(
    workspaceId,
    getFrontstageApiBaseUrl()
  );
}
