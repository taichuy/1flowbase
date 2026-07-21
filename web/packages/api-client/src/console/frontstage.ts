import { apiFetch, apiFetchBlob, apiFetchStream } from '../transport';

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

export interface ConsoleFrontstageInterfaceCapability {
  interface_id: string;
  method: string;
  path: string;
  name: string;
  short_description: string;
  parameter_schema: unknown;
  result_schema: unknown;
  request_media_type: string | null;
  response_media_type: string | null;
  schema_digest: string;
  adapter_id: string;
  host_injected_parameters: string[];
  scope: string;
  risk_level: string;
  authorization: string;
  bindable: boolean;
  disabled_reason: string | null;
}

export interface ConsoleFrontstageInterfaceCapabilitySummary {
  interface_id: string;
  method: string;
  path: string;
  adapter_id: string;
}

export interface ConsoleFrontstageInterfaceCapabilityPage {
  items: ConsoleFrontstageInterfaceCapabilitySummary[];
  total: number;
  offset: number;
  limit: number;
  has_more: boolean;
  next_offset: number | null;
  adapter_ids: string[];
  methods: string[];
}

export interface ConsoleFrontstageInterfaceCapabilityQuery {
  path_query?: string;
  adapter_id?: string;
  method?: string;
  offset?: number;
  limit?: number;
}

export interface FrontstageCallableBinaryResource {
  bytes: Uint8Array;
  file_name: string | null;
  content_type: string;
}

export interface FrontstageCallableEventStream<T> extends AsyncIterable<T> {
  cancel(): void;
}

export interface FrontstageCallableRequest {
  path?: Record<string, unknown>;
  query?: Record<string, unknown>;
  headers?: Record<string, unknown>;
  body?: unknown;
}

export interface DispatchFrontstageCallableInput {
  block_id: string;
  binding_alias: string;
  schema_digest: string;
  run_id: string;
  draft_hash: string;
  request?: FrontstageCallableRequest;
  write_grant?: string;
}

export interface IssueFrontstageCallableWriteGrantInput {
  block_id: string;
  binding_alias: string;
  schema_digest: string;
  run_id: string;
  draft_hash: string;
}

export interface FrontstageCallableWriteGrant {
  grant_token: string;
  expires_at: string;
}

export function listFrontstageInterfaceCapabilities(
  workspaceId: string,
  query: ConsoleFrontstageInterfaceCapabilityQuery = {},
  baseUrl?: string
): Promise<ConsoleFrontstageInterfaceCapabilityPage> {
  const params = new URLSearchParams();
  if (query.path_query) params.set('path_query', query.path_query);
  if (query.adapter_id) params.set('adapter_id', query.adapter_id);
  if (query.method) params.set('method', query.method);
  if (query.offset !== undefined) params.set('offset', String(query.offset));
  if (query.limit !== undefined) params.set('limit', String(query.limit));
  const suffix = params.size > 0 ? `?${params.toString()}` : '';
  return apiFetch<ConsoleFrontstageInterfaceCapabilityPage>({
    path: `/api/console/frontstage/${workspaceId}/interface-capabilities${suffix}`,
    method: 'GET',
    baseUrl
  });
}

export function getFrontstageInterfaceCapability(
  workspaceId: string,
  interfaceId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageInterfaceCapability> {
  return apiFetch<ConsoleFrontstageInterfaceCapability>({
    path: `/api/console/frontstage/${workspaceId}/interface-capabilities/${encodeURIComponent(interfaceId)}`,
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

export async function dispatchFrontstageCallableBinary(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: DispatchFrontstageCallableInput,
  csrfToken: string,
  baseUrl?: string
): Promise<FrontstageCallableBinaryResource> {
  const response = await apiFetchBlob({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${tabId}/callable-interfaces/dispatch`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
  return {
    bytes: new Uint8Array(await response.blob.arrayBuffer()),
    file_name: response.filename,
    content_type: response.contentType
  };
}

const MAX_SSE_BUFFER_LENGTH = 1024 * 1024;

export async function dispatchFrontstageCallableStream<T = unknown>(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: DispatchFrontstageCallableInput,
  csrfToken: string,
  baseUrl?: string
): Promise<FrontstageCallableEventStream<T>> {
  const response = await apiFetchStream({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${tabId}/callable-interfaces/dispatch`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl,
    headers: { accept: 'text/event-stream' }
  });
  return createSseIterable<T>(response.body, response.cancel);
}

function createSseIterable<T>(
  stream: ReadableStream<Uint8Array>,
  cancel: () => void
): FrontstageCallableEventStream<T> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  let closed = false;
  let nextQueue: Promise<void> = Promise.resolve();
  const close = () => {
    if (closed) return;
    closed = true;
    cancel();
    void reader.cancel().catch(() => undefined);
  };
  const readNext = async (): Promise<IteratorResult<T>> => {
    while (!closed) {
      const boundary = buffer.search(/\r?\n\r?\n/u);
      if (boundary >= 0) {
        const delimiter = /^\r\n/u.test(buffer.slice(boundary)) ? 4 : 2;
        const event = buffer.slice(0, boundary);
        buffer = buffer.slice(boundary + delimiter);
        const data = event
          .split(/\r?\n/u)
          .filter((line) => line.startsWith('data:'))
          .map((line) => line.slice(5).replace(/^ /u, ''))
          .join('\n');
        if (data.length > 0) {
          return { done: false, value: parseSseData(data) as T };
        }
        continue;
      }
      const chunk = await reader.read();
      if (chunk.done) {
        close();
        break;
      }
      buffer += decoder.decode(chunk.value, { stream: true });
      if (buffer.length > MAX_SSE_BUFFER_LENGTH) {
        close();
        throw new Error('Callable SSE event exceeded the 1 MiB limit.');
      }
    }
    return { done: true, value: undefined };
  };
  const iterator: AsyncIterator<T> = {
    next() {
      const result = nextQueue.then(readNext);
      nextQueue = result.then(
        () => undefined,
        () => undefined
      );
      return result;
    },
    async return() {
      close();
      return { done: true, value: undefined };
    }
  };
  return {
    cancel: close,
    [Symbol.asyncIterator]() {
      return iterator;
    }
  };
}

function parseSseData(value: string): unknown {
  try {
    return JSON.parse(value);
  } catch {
    return value;
  }
}

export function issueFrontstageCallableWriteGrant(
  workspaceId: string,
  pageId: string,
  tabId: string,
  input: IssueFrontstageCallableWriteGrantInput,
  csrfToken: string,
  baseUrl?: string
): Promise<FrontstageCallableWriteGrant> {
  return apiFetch<FrontstageCallableWriteGrant>({
    path: `/api/console/frontstage/${workspaceId}/pages/${pageId}/tabs/${tabId}/callable-interfaces/write-grants`,
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
