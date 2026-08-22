import {
  createFrontstagePageTab as createPageTab,
  deleteFrontstagePageTab as deletePageTab,
  listFrontstagePageTabs,
  updateFrontstagePageTab,
  type ConsoleFrontstagePageTab
} from '@1flowbase/api-client';

import { getFrontstageApiBaseUrl } from './page-tree';

export type FrontstagePageTab = ConsoleFrontstagePageTab;

export interface CreateFrontstagePageTabInput {
  title: string;
  route_segment: string;
  rank: string;
}

export interface RenameFrontstagePageTabInput {
  title: string | null;
}

export const frontstagePageTabsQueryKey = (
  workspaceId: string,
  pageId: string
) => ['frontstage', workspaceId, 'pages', pageId, 'tabs'] as const;

export function fetchFrontstagePageTabs(
  workspaceId: string,
  pageId: string
): Promise<FrontstagePageTab[]> {
  return listFrontstagePageTabs(pageId, getFrontstageApiBaseUrl());
}

export function createFrontstagePageTab(
  workspaceId: string,
  pageId: string,
  input: CreateFrontstagePageTabInput,
  csrfToken: string
): Promise<FrontstagePageTab> {
  return createPageTab(
    pageId,
    input,
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}

export function renameFrontstagePageTab(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: RenameFrontstagePageTabInput,
  csrfToken: string
): Promise<FrontstagePageTab> {
  return updateFrontstagePageTab(
    pageId,
    tabId,
    input,
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}

export function moveFrontstagePageTab(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: { rank: string },
  csrfToken: string
): Promise<FrontstagePageTab> {
  return updateFrontstagePageTab(
    pageId,
    tabId,
    input,
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}

export function deleteFrontstagePageTab(
  workspaceId: string,
  pageId: string,
  tabId: string,
  csrfToken: string
): Promise<void> {
  return deletePageTab(
    pageId,
    tabId,
    csrfToken,
    getFrontstageApiBaseUrl()
  );
}
