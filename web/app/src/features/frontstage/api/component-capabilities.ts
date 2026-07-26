import {
  getFrontstageComponentCapability,
  listFrontstageComponentCapabilities,
  type ConsoleFrontstageComponentCapability,
  type ConsoleFrontstageComponentCapabilityPage,
  type ConsoleFrontstageComponentCapabilityQuery
} from '@1flowbase/api-client';

import { getFrontstageApiBaseUrl } from './page-tree';

export type FrontstageComponentCapability =
  ConsoleFrontstageComponentCapability;
export type FrontstageComponentCapabilityPage =
  ConsoleFrontstageComponentCapabilityPage;
export type FrontstageComponentCapabilityQuery =
  ConsoleFrontstageComponentCapabilityQuery;

export function fetchFrontstageComponentCapabilities(
  workspaceId: string,
  query: FrontstageComponentCapabilityQuery
): Promise<FrontstageComponentCapabilityPage> {
  return listFrontstageComponentCapabilities(
    workspaceId,
    query,
    getFrontstageApiBaseUrl()
  );
}

export function fetchFrontstageComponentCapability(
  workspaceId: string,
  componentId: string
): Promise<FrontstageComponentCapability> {
  return getFrontstageComponentCapability(
    workspaceId,
    componentId,
    getFrontstageApiBaseUrl()
  );
}
