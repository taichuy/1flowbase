import {
  apiFetch,
  type ConsoleFrontstagePageNode
} from '@1flowbase/api-client';

import { getFrontstageApiBaseUrl } from './page-tree';

export interface FrontstagePageContentNode {
  id: string;
  title: string | null;
  icon?: string | null;
  tooltip?: string | null;
  kind: 'group' | 'page';
  parentId: string | null;
  rank: string;
}

export interface FrontstagePageSchema {
  rootUid: string;
  payload: unknown;
}

export interface FrontstagePageRoot {
  uid: string;
  payload: unknown;
}

export interface FrontstagePageContent {
  page: FrontstagePageContentNode;
  schema: FrontstagePageSchema;
  root: FrontstagePageRoot;
}

export interface SaveFrontstagePageContentPayloadInput {
  payload: unknown;
}

export interface SaveFrontstagePageContentInput {
  schema: SaveFrontstagePageContentPayloadInput;
  root: SaveFrontstagePageContentPayloadInput;
}

interface FrontstagePageDetailDto {
  page: Omit<ConsoleFrontstagePageNode, 'schema_root_uid'>;
  tab: {
    id: string;
    page_id: string;
    title: string | null;
    rank: string;
    is_default: boolean;
    document_root_uid: string;
  };
  schema: {
    root_uid: string;
    payload: unknown;
  };
  root: {
    uid: string;
    payload: unknown;
  };
}

export const frontstagePageContentQueryKey = (
  workspaceId: string,
  pageId: string,
  tabId: string
) => ['frontstage', workspaceId, 'pages', pageId, 'tabs', tabId, 'content'] as const;

function mapFrontstagePageNode(
  page: FrontstagePageDetailDto['page']
): FrontstagePageContentNode {
  return {
    id: page.id,
    title: page.title,
    icon: page.icon,
    tooltip: page.tooltip,
    kind: page.kind,
    parentId: page.parent_id,
    rank: page.rank
  };
}

function mapFrontstagePageContent(
  detail: FrontstagePageDetailDto
): FrontstagePageContent {
  return {
    page: mapFrontstagePageNode(detail.page),
    schema: {
      rootUid: detail.schema.root_uid,
      payload: detail.schema.payload
    },
    root: {
      uid: detail.root.uid,
      payload: detail.root.payload
    }
  };
}

export async function fetchFrontstagePageContent(
  workspaceId: string,
  pageId: string,
  tabId: string
): Promise<FrontstagePageContent> {
  const detail = await apiFetch<FrontstagePageDetailDto>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${tabId}`,
    method: 'GET',
    baseUrl: getFrontstageApiBaseUrl()
  });

  return mapFrontstagePageContent(detail);
}

export async function saveFrontstagePageContent(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: SaveFrontstagePageContentInput,
  csrfToken: string
): Promise<FrontstagePageContent> {
  const detail = await apiFetch<FrontstagePageDetailDto>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${tabId}/document`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl: getFrontstageApiBaseUrl()
  });

  return mapFrontstagePageContent(detail);
}
