import {
  getFrontstageInterfaceCapability,
  listFrontstageInterfaceCapabilities,
  type ConsoleFrontstageInterfaceCapability,
  type ConsoleFrontstageInterfaceCapabilityPage,
  type ConsoleFrontstageInterfaceCapabilityQuery
} from '@1flowbase/api-client';

import { getFrontstageApiBaseUrl } from './page-tree';

export type FrontstageInterfaceCapability =
  ConsoleFrontstageInterfaceCapability;
export type FrontstageInterfaceCapabilityPage =
  ConsoleFrontstageInterfaceCapabilityPage;
export type FrontstageInterfaceCapabilityQuery =
  ConsoleFrontstageInterfaceCapabilityQuery;

export function fetchFrontstageInterfaceCapabilities(
  workspaceId: string,
  query: FrontstageInterfaceCapabilityQuery
): Promise<FrontstageInterfaceCapabilityPage> {
  return listFrontstageInterfaceCapabilities(
    workspaceId,
    query,
    getFrontstageApiBaseUrl()
  );
}

export function fetchFrontstageInterfaceCapability(
  workspaceId: string,
  interfaceId: string
): Promise<FrontstageInterfaceCapability> {
  return getFrontstageInterfaceCapability(
    workspaceId,
    interfaceId,
    getFrontstageApiBaseUrl()
  );
}
