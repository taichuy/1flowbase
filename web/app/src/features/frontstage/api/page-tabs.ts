import { apiFetch } from '@1flowbase/api-client';

import { getFrontstageApiBaseUrl } from './page-tree';

export interface FrontstagePageTab {
  id: string;
  page_id: string;
  title: string | null;
  rank: string;
  is_default: boolean;
  document_root_uid: string;
}

export interface CreateFrontstagePageTabInput {
  title: string | null;
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
  return apiFetch<FrontstagePageTab[]>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs`,
    method: 'GET',
    baseUrl: getFrontstageApiBaseUrl()
  });
}

export function createFrontstagePageTab(
  workspaceId: string,
  pageId: string,
  input: CreateFrontstagePageTabInput,
  csrfToken: string
): Promise<FrontstagePageTab> {
  return apiFetch<FrontstagePageTab>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl: getFrontstageApiBaseUrl()
  });
}

export function renameFrontstagePageTab(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: RenameFrontstagePageTabInput,
  csrfToken: string
): Promise<FrontstagePageTab> {
  return apiFetch<FrontstagePageTab>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${tabId}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl: getFrontstageApiBaseUrl()
  });
}

export function moveFrontstagePageTab(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: { rank: string },
  csrfToken: string
): Promise<FrontstagePageTab> {
  return apiFetch<FrontstagePageTab>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${tabId}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl: getFrontstageApiBaseUrl()
  });
}

export function deleteFrontstagePageTab(
  workspaceId: string,
  pageId: string,
  tabId: string,
  csrfToken: string
): Promise<void> {
  return apiFetch<void>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${tabId}`,
    method: 'DELETE',
    csrfToken,
    baseUrl: getFrontstageApiBaseUrl()
  });
}
