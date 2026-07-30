import {
  createFrontstageBlock,
  getFrontstagePageTabDetail,
  saveFrontstageTabDocument,
  type ConsoleFrontstagePageDetail,
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
  contentPresentation: 'single' | 'tabs';
}

export interface FrontstageTabDocument {
  rootUid: string;
  payload: unknown;
}

export interface FrontstagePageContentTab {
  id: string;
  pageId: string;
  title: string | null;
  rank: string;
  isDefault: boolean;
  routeSegment: string | null;
  documentRootUid: string;
}

export interface FrontstagePageContent {
  page: FrontstagePageContentNode;
  tab: FrontstagePageContentTab;
  document: FrontstageTabDocument;
}

export interface SaveFrontstageTabDocumentInput {
  payload: unknown;
}

export interface CreateFrontstageBlockInput {
  payload: unknown;
  code_ref: string;
  code: string;
}

type FrontstagePageDetailDto = ConsoleFrontstagePageDetail;

export const frontstagePageContentQueryKey = (
  workspaceId: string,
  pageId: string,
  tabId: string
) => ['frontstage', workspaceId, 'pages', pageId, 'tabs', tabId, 'content'] as const;

function mapFrontstagePageNode(
  page: ConsoleFrontstagePageNode
): FrontstagePageContentNode {
  return {
    id: page.id,
    title: page.title,
    icon: page.icon,
    tooltip: page.tooltip,
    kind: page.kind,
    parentId: page.parent_id,
    rank: page.rank,
    contentPresentation: page.content_presentation
  };
}

function mapFrontstagePageContentTab(
  tab: FrontstagePageDetailDto['tab']
): FrontstagePageContentTab {
  return {
    id: tab.id,
    pageId: tab.page_id,
    title: tab.title,
    rank: tab.rank,
    isDefault: tab.is_default,
    routeSegment: tab.route_segment,
    documentRootUid: tab.document_root_uid
  };
}

function mapFrontstagePageContent(
  detail: FrontstagePageDetailDto
): FrontstagePageContent {
  return {
    page: mapFrontstagePageNode(detail.page),
    tab: mapFrontstagePageContentTab(detail.tab),
    document: {
      rootUid: detail.document.root_uid,
      payload: detail.document.payload
    }
  };
}

export async function fetchFrontstagePageContent(
  workspaceId: string,
  pageId: string,
  tabReference: string
): Promise<FrontstagePageContent> {
  const detail = await getFrontstagePageTabDetail(
    workspaceId,
    pageId,
    tabReference,
    getFrontstageApiBaseUrl()
  );

  return mapFrontstagePageContent(detail);
}

export async function saveFrontstagePageContent(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: SaveFrontstageTabDocumentInput,
  csrfToken: string
): Promise<FrontstagePageContent> {
  const detail = await saveFrontstageTabDocument(
    workspaceId,
    pageId,
    tabId,
    input,
    csrfToken,
    getFrontstageApiBaseUrl()
  );

  return mapFrontstagePageContent(detail);
}

export async function createFrontstagePageBlock(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: CreateFrontstageBlockInput,
  csrfToken: string
): Promise<FrontstagePageContent> {
  const detail = await createFrontstageBlock(
    workspaceId,
    pageId,
    tabId,
    input,
    csrfToken,
    getFrontstageApiBaseUrl()
  );

  return mapFrontstagePageContent(detail);
}
