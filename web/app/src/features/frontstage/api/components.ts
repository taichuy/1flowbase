import {
  getFrontstageComponent,
  listFrontstageComponents,
  type ConsoleFrontstageComponent,
  type ConsoleFrontstageComponentPage,
  type ConsoleFrontstageComponentQuery
} from '@1flowbase/api-client';
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
