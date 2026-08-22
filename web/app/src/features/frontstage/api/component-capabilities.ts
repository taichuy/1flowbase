import {
  getFrontstageComponentCapability,
  listFrontstageComponentCapabilities,
  resolveFrontstageComponentDependencyLock as requestFrontstageComponentDependencyLock,
  type ConsoleFrontstageComponentCapability,
  type ConsoleFrontstageComponentCapabilityPage,
  type ConsoleFrontstageComponentCapabilityQuery
} from '@1flowbase/api-client';
import {
  canonicalizeNativeReactCatalogDependencyLock,
  type NativeReactCatalogDependencyLock
} from '@1flowbase/page-runtime';

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
    query,
    getFrontstageApiBaseUrl()
  );
}

export function fetchFrontstageComponentCapability(
  workspaceId: string,
  componentId: string
): Promise<FrontstageComponentCapability> {
  return getFrontstageComponentCapability(
    componentId,
    getFrontstageApiBaseUrl()
  );
}

export async function resolveFrontstageComponentDependencyLock(
  workspaceId: string,
  sourceCode: string
): Promise<NativeReactCatalogDependencyLock> {
  const result = await requestFrontstageComponentDependencyLock(
    sourceCode,
    getFrontstageApiBaseUrl()
  );
  const dependencyLock = canonicalizeNativeReactCatalogDependencyLock(
    result.dependency_lock
  );
  if (!dependencyLock) {
    throw new Error('Frontstage component dependency lock is invalid.');
  }
  return dependencyLock;
}
