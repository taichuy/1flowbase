import { apiFetch } from '../transport';

export type ConsoleFrontstageNavigationPlacement = 'topbar' | 'sidebar';
export type ConsoleFrontstagePageContentPresentation = 'single' | 'tabs';

export interface ConsoleFrontstagePageTreeNode {
  id: string;
  title: string | null;
  icon?: string | null;
  tooltip?: string | null;
  is_hidden?: boolean;
  placement: ConsoleFrontstageNavigationPlacement;
  content_presentation: ConsoleFrontstagePageContentPresentation;
  slug?: string | null;
  kind: 'group' | 'page';
  children: ConsoleFrontstagePageTreeNode[];
}

export interface ConsoleFrontstagePageNode {
  id: string;
  title: string | null;
  icon?: string | null;
  tooltip?: string | null;
  is_hidden?: boolean;
  placement?: ConsoleFrontstageNavigationPlacement;
  content_presentation: ConsoleFrontstagePageContentPresentation;
  slug?: string | null;
  kind: 'group' | 'page';
  parent_id: string | null;
  rank: string;
}

export interface ConsoleFrontstagePageTab {
  id: string;
  page_id: string;
  title: string | null;
  rank: string;
  is_default: boolean;
  route_segment: string | null;
  document_root_uid: string;
}

export interface ConsoleFrontstageTabDocument {
  root_uid: string;
  payload: unknown;
}

export interface ConsoleFrontstagePageDetail {
  page: ConsoleFrontstagePageNode;
  tab: ConsoleFrontstagePageTab;
  document: ConsoleFrontstageTabDocument;
}

export interface ConsoleFrontstagePageCreationResponse {
  page: ConsoleFrontstagePageNode;
  default_tab: ConsoleFrontstagePageTab;
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
  placement?: ConsoleFrontstageNavigationPlacement;
  slug?: string | null;
}

export interface UpdateFrontstagePageNodeTitleInput {
  title?: string | null;
  icon?: string | null;
  tooltip?: string | null;
  is_hidden?: boolean;
  placement?: ConsoleFrontstageNavigationPlacement;
  content_presentation?: ConsoleFrontstagePageContentPresentation;
  slug?: string | null;
}

export interface MoveFrontstagePageNodeInput {
  parent_id?: string | null;
  rank?: string | null;
}

export interface CreateFrontstagePageTabInput {
  title: string;
  route_segment: string;
  rank?: string;
}

export interface UpdateFrontstagePageTabInput {
  title?: string | null;
  rank?: string;
}

export interface SaveFrontstageTabDocumentInput {
  payload: unknown;
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

export type ConsoleFrontstageCallableParameterLocation =
  | 'path'
  | 'query'
  | 'header'
  | 'body';

export interface ConsoleFrontstageCallableParameter {
  name: string;
  field_type: string;
  location: ConsoleFrontstageCallableParameterLocation;
  description: string | null;
  required: boolean;
  schema: unknown;
}

export interface ConsoleFrontstageCallableInterface {
  operation_id: string;
  method: string;
  path: string;
  name: string;
  description: string;
  parameters: ConsoleFrontstageCallableParameter[];
  request_schema: unknown;
  response_schema: unknown;
  schema_digest: string;
  adapter_id: string;
  host_injected_parameters: string[];
  scope: string;
  risk_level: string;
  authorization: string;
  bindable: boolean;
  disabled_reason: string | null;
}

export interface FrontstageCallableRequest {
  path?: Record<string, unknown>;
  query?: Record<string, unknown>;
  headers?: Record<string, unknown>;
  body?: unknown;
}

export interface DispatchFrontstageCallableInput {
  operation_id: string;
  request?: FrontstageCallableRequest;
  run_authorization?: {
    run_id: string;
    operation_id: string;
    confirmed: boolean;
  };
}

export function listFrontstageCallableInterfaces(
  workspaceId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageCallableInterface[]> {
  return apiFetch<ConsoleFrontstageCallableInterface[]>({
    path: `/api/console/frontstage/${workspaceId}/callable-interfaces`,
    method: 'GET',
    baseUrl
  });
}

export function dispatchFrontstageCallable<T = unknown>(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: DispatchFrontstageCallableInput,
  csrfToken: string,
  baseUrl?: string
): Promise<T> {
  return apiFetch<T>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${tabId}/callable-interfaces/dispatch`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
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

export function getFrontstagePageTabDetail(
  workspaceId: string,
  pageId: string,
  tabReference: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageDetail> {
  return apiFetch<ConsoleFrontstagePageDetail>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${encodeURIComponent(tabReference)}`,
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
): Promise<ConsoleFrontstagePageCreationResponse> {
  return apiFetch<ConsoleFrontstagePageCreationResponse>({
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

export function listFrontstagePageTabs(
  workspaceId: string,
  pageId: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageTab[]> {
  return apiFetch<ConsoleFrontstagePageTab[]>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs`,
    method: 'GET',
    baseUrl
  });
}

export function createFrontstagePageTab(
  workspaceId: string,
  pageId: string,
  input: CreateFrontstagePageTabInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageTab> {
  return apiFetch<ConsoleFrontstagePageTab>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateFrontstagePageTab(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: UpdateFrontstagePageTabInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageTab> {
  return apiFetch<ConsoleFrontstagePageTab>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${tabId}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteFrontstagePageTab(
  workspaceId: string,
  pageId: string,
  tabId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetch<void>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${tabId}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function saveFrontstageTabDocument(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: SaveFrontstageTabDocumentInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageDetail> {
  return apiFetch<ConsoleFrontstagePageDetail>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${tabId}/document`,
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

export interface ConsoleFrontstageDataCapabilityField {
  code: string;
  title: string;
  field_kind: string;
  is_required: boolean;
  is_writable: boolean;
}

export interface ConsoleFrontstageDataCapabilityModel {
  code: string;
  scope_kind: string;
  fields: ConsoleFrontstageDataCapabilityField[];
}

export interface ConsoleFrontstageDataCapabilityDescriptor {
  id: string;
  kind: string;
  params_schema: unknown;
  result_schema: unknown;
}

export interface ConsoleFrontstageDataCapabilities {
  queries: ConsoleFrontstageDataCapabilityDescriptor[];
  actions: ConsoleFrontstageDataCapabilityDescriptor[];
  models: ConsoleFrontstageDataCapabilityModel[];
}

export function listFrontstageDataCapabilities(
  workspaceId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageDataCapabilities> {
  return apiFetch<ConsoleFrontstageDataCapabilities>({
    path: `/api/console/frontstage/${workspaceId}/data-capabilities`,
    method: 'GET',
    baseUrl
  });
}
