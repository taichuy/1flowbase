import {
  listFrontstageCallableInterfaces,
  type ConsoleFrontstageCallableInterface
} from '@1flowbase/api-client';

import { getFrontstageApiBaseUrl } from './page-tree';

export type FrontstageCallableInterface = ConsoleFrontstageCallableInterface;

export function fetchFrontstageCallableInterfaces(
  workspaceId: string
): Promise<FrontstageCallableInterface[]> {
  return listFrontstageCallableInterfaces(workspaceId, getFrontstageApiBaseUrl());
}
