import { apiFetch } from '../transport';

export interface ConsoleFrontstagePageTreeNode {
  id: string;
  title: string | null;
  icon?: string | null;
  tooltip?: string | null;
  is_hidden?: boolean;
  kind: 'group' | 'page';
  children: ConsoleFrontstagePageTreeNode[];
}

export interface ConsoleFrontstagePageNode {
  id: string;
  title: string | null;
  icon?: string | null;
  tooltip?: string | null;
  is_hidden?: boolean;
  kind: 'group' | 'page';
  parent_id: string | null;
  rank: string;
  schema_root_uid: string | null;
}

export interface ConsoleFrontstagePageSchema {
  root_uid: string;
  payload: unknown;
}

export interface ConsoleFrontstagePageRoot {
  uid: string;
  payload: unknown;
}

export interface ConsoleFrontstagePageDetail {
  page: ConsoleFrontstagePageNode;
  schema: ConsoleFrontstagePageSchema;
  root: ConsoleFrontstagePageRoot;
}

export interface ConsoleFrontstageBlockCode {
  page_id: string;
  code_ref: string;
  code: string;
}

export interface CreateFrontstagePageNodeInput {
  title?: string | null;
  icon?: string | null;
  tooltip?: string | null;
  parent_id?: string | null;
  rank?: string | null;
}

export interface UpdateFrontstagePageNodeTitleInput {
  title?: string | null;
  icon?: string | null;
  tooltip?: string | null;
  is_hidden?: boolean;
}

export interface MoveFrontstagePageNodeInput {
  parent_id?: string | null;
  rank?: string | null;
}

export interface SaveFrontstagePageContentPayloadInput {
  payload: unknown;
}

export interface SaveFrontstagePageContentInput {
  schema: SaveFrontstagePageContentPayloadInput;
  root: SaveFrontstagePageContentPayloadInput;
}

export interface SaveFrontstageBlockCodeInput {
  code: string;
}

export interface DispatchFrontstageQueryInput {
  query_id: string;
  params?: unknown;
}

export interface DispatchFrontstageActionInput {
  action_id: string;
  params?: unknown;
}

export function dispatchFrontstageQuery<T = unknown>(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: DispatchFrontstageQueryInput,
  baseUrl?: string
): Promise<T> {
  return apiFetch<T>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${tabId}/queries/dispatch`,
    method: 'POST',
    body: input,
    baseUrl
  });
}

export function dispatchFrontstageAction<T = unknown>(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: DispatchFrontstageActionInput,
  csrfToken: string,
  baseUrl?: string
): Promise<T> {
  return apiFetch<T>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${tabId}/actions/dispatch`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function listFrontstagePages(
  workspaceId: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageTreeNode[]> {
  return apiFetch<ConsoleFrontstagePageTreeNode[]>({
    path: `/api/console/frontstage/${workspaceId}/pages`,
    method: 'GET',
    baseUrl
  });
}

export function getFrontstagePageDetail(
  workspaceId: string,
  pageId: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageDetail> {
  return apiFetch<ConsoleFrontstagePageDetail>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}`,
    method: 'GET',
    baseUrl
  });
}

export function createFrontstageGroup(
  workspaceId: string,
  input: CreateFrontstagePageNodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageNode> {
  return apiFetch<ConsoleFrontstagePageNode>({
    path: `/api/console/frontstage/${workspaceId}/pages/groups`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function createFrontstagePage(
  workspaceId: string,
  input: CreateFrontstagePageNodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageNode> {
  return apiFetch<ConsoleFrontstagePageNode>({
    path: `/api/console/frontstage/${workspaceId}/pages`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateFrontstagePageNodeTitle(
  workspaceId: string,
  pageNodeId: string,
  input: UpdateFrontstagePageNodeTitleInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageNode> {
  return apiFetch<ConsoleFrontstagePageNode>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageNodeId}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function moveFrontstagePageNode(
  workspaceId: string,
  pageNodeId: string,
  input: MoveFrontstagePageNodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageNode> {
  return apiFetch<ConsoleFrontstagePageNode>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageNodeId}/move`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteFrontstagePageNode(
  workspaceId: string,
  pageNodeId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetch<void>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageNodeId}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function saveFrontstagePageContent(
  workspaceId: string,
  pageId: string,
  input: SaveFrontstagePageContentInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageDetail> {
  return apiFetch<ConsoleFrontstagePageDetail>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/content`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function getFrontstageBlockCode(
  workspaceId: string,
  pageId: string,
  codeRef: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockCode> {
  const encodedCodeRef = encodeURIComponent(codeRef);

  return apiFetch<ConsoleFrontstageBlockCode>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/block-codes/${encodedCodeRef}`,
    method: 'GET',
    baseUrl
  });
}

export function saveFrontstageBlockCode(
  workspaceId: string,
  pageId: string,
  codeRef: string,
  input: SaveFrontstageBlockCodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstageBlockCode> {
  const encodedCodeRef = encodeURIComponent(codeRef);

  return apiFetch<ConsoleFrontstageBlockCode>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/block-codes/${encodedCodeRef}`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}
