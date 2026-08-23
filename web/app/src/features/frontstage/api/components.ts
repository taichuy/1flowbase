import {
  getFrontstageComponent,
  listFrontstageComponents,
  resolveFrontstageComponentDependencyLock as requestFrontstageComponentDependencyLock,
  type ConsoleFrontstageComponent,
  type ConsoleFrontstageComponentPage,
  type ConsoleFrontstageComponentQuery
} from '@1flowbase/api-client';
import {
  canonicalizeNativeReactCatalogDependencyLock,
  type NativeReactCatalogDependencyLock
} from '@1flowbase/page-runtime';

import { getFrontstageApiBaseUrl } from './page-tree';

export type FrontstageComponent = ConsoleFrontstageComponent;
export type FrontstageComponentPage = ConsoleFrontstageComponentPage;
export type FrontstageComponentQuery = ConsoleFrontstageComponentQuery;

export function fetchFrontstageComponents(
  workspaceId: string,
  query: FrontstageComponentQuery
): Promise<FrontstageComponentPage> {
  return listFrontstageComponents(query, getFrontstageApiBaseUrl());
}

export function fetchFrontstageComponent(
  workspaceId: string,
  componentId: string
): Promise<FrontstageComponent> {
  return getFrontstageComponent(componentId, getFrontstageApiBaseUrl());
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
