import { apiFetch, apiFetchResource, apiFetchStream } from '../transport';

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
  path_prefixes?: string[];
  path_query?: string;
  adapter_id?: string;
  method?: string;
  offset?: number;
  limit?: number;
}

export interface ConsoleFrontstageComponentUpstream {
  identity: string;
  version: string;
}

export interface ConsoleFrontstageComponent {
  id: string;
  scope_id: string;
  component_code: string;
  name: string;
  description: string;
  import_code: string;
  source_code: string;
  origin: 'official' | 'custom';
  source: string;
  group: string;
  upstream: ConsoleFrontstageComponentUpstream;
  version: string;
  keywords: string[];
  catalog_updated_at: string | null;
  source_locator: string | null;
  source_checksum: string | null;
  created_at: string;
  updated_at: string;
}

export interface ConsoleFrontstageComponentPage {
  items: ConsoleFrontstageComponent[];
  total: number;
  offset: number;
  limit: number;
  has_more: boolean;
  next_offset: number | null;
}

export interface ConsoleFrontstageComponentQuery {
  query?: string;
  offset?: number;
  limit?: number;
}

export interface ConsoleFrontstageComponentDependencyLock {
  dependency_lock: unknown;
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
  method: string;
  path: string;
  request?: FrontstageCallableRequest;
}

export function listFrontstageInterfaceCapabilities(
  query: ConsoleFrontstageInterfaceCapabilityQuery = {},
  baseUrl?: string
): Promise<ConsoleFrontstageInterfaceCapabilityPage> {
  const params = new URLSearchParams();
  if (query.path_prefixes?.length) {
    params.set('path_prefixes', query.path_prefixes.join(','));
  }
  if (query.path_query) params.set('path_query', query.path_query);
  if (query.adapter_id) params.set('adapter_id', query.adapter_id);
  if (query.method) params.set('method', query.method);
  if (query.offset !== undefined) params.set('offset', String(query.offset));
  if (query.limit !== undefined) params.set('limit', String(query.limit));
  const suffix = params.size > 0 ? `?${params.toString()}` : '';
  return apiFetch<ConsoleFrontstageInterfaceCapabilityPage>({
    path: `/api/console/frontstage/interface-capabilities${suffix}`,
    method: 'GET',
    baseUrl
  });
}

export function getFrontstageInterfaceCapability(
  interfaceId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageInterfaceCapability> {
  return apiFetch<ConsoleFrontstageInterfaceCapability>({
    path: `/api/console/frontstage/interface-capabilities/${encodeURIComponent(interfaceId)}`,
    method: 'GET',
    baseUrl
  });
}

export function listFrontstageComponents(
  query: ConsoleFrontstageComponentQuery = {},
  baseUrl?: string
): Promise<ConsoleFrontstageComponentPage> {
  const params = new URLSearchParams();
  if (query.query) params.set('query', query.query);
  if (query.offset !== undefined) params.set('offset', String(query.offset));
  if (query.limit !== undefined) params.set('limit', String(query.limit));
  const suffix = params.size > 0 ? `?${params.toString()}` : '';
  return apiFetch<ConsoleFrontstageComponentPage>({
    path: `/api/console/frontstage/components${suffix}`,
    method: 'GET',
    baseUrl
  });
}

export function getFrontstageComponent(
  componentId: string,
  baseUrl?: string
): Promise<ConsoleFrontstageComponent> {
  return apiFetch<ConsoleFrontstageComponent>({
    path: `/api/console/frontstage/components/${encodeURIComponent(componentId)}`,
    method: 'GET',
    baseUrl
  });
}

export function resolveFrontstageComponentDependencyLock(
  sourceCode: string,
  baseUrl?: string
): Promise<ConsoleFrontstageComponentDependencyLock> {
  return apiFetch<ConsoleFrontstageComponentDependencyLock>({
    path: '/api/console/frontstage/component-dependency-lock',
    method: 'POST',
    body: { source_code: sourceCode },
    baseUrl
  });
}

export function frontstageComponentModuleAssetPath(sha256: string): string {
  return `/api/console/frontstage/component-module-assets/${encodeURIComponent(sha256)}`;
}

export function dispatchFrontstageCallable<T = unknown>(
  pageId: string,
  tabId: string,
  input: DispatchFrontstageCallableInput,
  csrfToken: string,
  baseUrl?: string
): Promise<T> {
  return dispatchFrontstageCallableResource<T>({
    path: `/api/console/frontstage/pages/${pageId}/tabs/${tabId}/callable-interfaces/dispatch`,
    input,
    csrfToken,
    baseUrl
  });
}

async function dispatchFrontstageCallableResource<T>({
  path,
  input,
  csrfToken,
  baseUrl
}: {
  path: string;
  input: DispatchFrontstageCallableInput;
  csrfToken: string;
  baseUrl?: string;
}): Promise<T> {
  const response = await apiFetchResource<T>({
    path,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
  if (response.kind === 'json') return response.value;
  if (response.kind === 'no_content') return undefined as T;
  return {
    bytes: new Uint8Array(await response.blob.arrayBuffer()),
    file_name: response.filename,
    content_type: response.contentType
  } as T;
}

const MAX_SSE_BUFFER_LENGTH = 1024 * 1024;

export async function dispatchFrontstageCallableStream<T = unknown>(
  pageId: string,
  tabId: string,
  input: DispatchFrontstageCallableInput,
  csrfToken: string,
  baseUrl?: string
): Promise<FrontstageCallableEventStream<T>> {
  const response = await apiFetchStream({
    path: `/api/console/frontstage/pages/${pageId}/tabs/${tabId}/callable-interfaces/dispatch`,
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

export function dispatchFrontstageQuery<T = unknown>(
  pageId: string,
  tabId: string,
  input: DispatchFrontstageQueryInput,
  baseUrl?: string
): Promise<T> {
  return apiFetch<T>({
    path: `/api/console/frontstage/pages/${pageId}/tabs/${tabId}/queries/dispatch`,
    method: 'POST',
    body: input,
    baseUrl
  });
}

export function dispatchFrontstageAction<T = unknown>(
  pageId: string,
  tabId: string,
  input: DispatchFrontstageActionInput,
  csrfToken: string,
  baseUrl?: string
): Promise<T> {
  return apiFetch<T>({
    path: `/api/console/frontstage/pages/${pageId}/tabs/${tabId}/actions/dispatch`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function listFrontstagePages(
  baseUrl?: string
): Promise<ConsoleFrontstagePageTreeNode[]> {
  return apiFetch<ConsoleFrontstagePageTreeNode[]>({
    path: '/api/console/frontstage/pages',
    method: 'GET',
    baseUrl
  });
}

export function getFrontstagePageTabDetail(
  pageId: string,
  tabReference: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageDetail> {
  return apiFetch<ConsoleFrontstagePageDetail>({
    path: `/api/console/frontstage/pages/${pageId}/tabs/${encodeURIComponent(tabReference)}`,
    method: 'GET',
    baseUrl
  });
}

export function createFrontstageGroup(
  input: CreateFrontstagePageNodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageNode> {
  return apiFetch<ConsoleFrontstagePageNode>({
    path: '/api/console/frontstage/pages/groups',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function createFrontstagePage(
  input: CreateFrontstagePageNodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageCreationResponse> {
  return apiFetch<ConsoleFrontstagePageCreationResponse>({
    path: '/api/console/frontstage/pages',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateFrontstagePageNodeTitle(
  pageNodeId: string,
  input: UpdateFrontstagePageNodeTitleInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageNode> {
  return apiFetch<ConsoleFrontstagePageNode>({
    path: `/api/console/frontstage/pages/${pageNodeId}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function moveFrontstagePageNode(
  pageNodeId: string,
  input: MoveFrontstagePageNodeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageNode> {
  return apiFetch<ConsoleFrontstagePageNode>({
    path: `/api/console/frontstage/pages/${pageNodeId}/move`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteFrontstagePageNode(
  pageNodeId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetch<void>({
    path: `/api/console/frontstage/pages/${pageNodeId}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function listFrontstagePageTabs(
  pageId: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageTab[]> {
  return apiFetch<ConsoleFrontstagePageTab[]>({
    path: `/api/console/frontstage/pages/${pageId}/tabs`,
    method: 'GET',
    baseUrl
  });
}

export function createFrontstagePageTab(
  pageId: string,
  input: CreateFrontstagePageTabInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageTab> {
  return apiFetch<ConsoleFrontstagePageTab>({
    path: `/api/console/frontstage/pages/${pageId}/tabs`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateFrontstagePageTab(
  pageId: string,
  tabId: string,
  input: UpdateFrontstagePageTabInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageTab> {
  return apiFetch<ConsoleFrontstagePageTab>({
    path: `/api/console/frontstage/pages/${pageId}/tabs/${tabId}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteFrontstagePageTab(
  pageId: string,
  tabId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetch<void>({
    path: `/api/console/frontstage/pages/${pageId}/tabs/${tabId}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function saveFrontstageTabDocument(
  pageId: string,
  tabId: string,
  input: SaveFrontstageTabDocumentInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleFrontstagePageDetail> {
  return apiFetch<ConsoleFrontstagePageDetail>({
    path: `/api/console/frontstage/pages/${pageId}/tabs/${tabId}/document`,
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
  baseUrl?: string
): Promise<ConsoleFrontstageDataCapabilities> {
  return apiFetch<ConsoleFrontstageDataCapabilities>({
    path: '/api/console/frontstage/data-capabilities',
    method: 'GET',
    baseUrl
  });
}

export interface ConsoleFrontstageUiTemplate {
  template_id: string | null;
  provider_code: string;
  contribution_code: string;
  name: string;
  source: string;
  language: 'jsx' | 'tsx';
  version: string;
  is_official: boolean;
  is_default: boolean;
}

export function listFrontstageUiTemplates(
  baseUrl?: string
): Promise<ConsoleFrontstageUiTemplate[]> {
  return apiFetch<ConsoleFrontstageUiTemplate[]>({
    path: '/api/console/frontstage/ui-templates',
    method: 'GET',
    baseUrl
  });
}
