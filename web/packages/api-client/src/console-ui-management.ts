import { apiFetch } from './transport';

export type UiCodeTemplateLanguage = 'jsx' | 'tsx';
export type UiComponentOrigin = 'official' | 'custom';

export interface ConsoleUiTemplateRevision {
  revision: number;
  source: string;
  language: UiCodeTemplateLanguage;
  is_published: boolean;
}
export interface ConsoleUiManagedTemplate {
  id: string;
  provider_code: string;
  contribution_code: string;
  name: string;
  latest_revision: ConsoleUiTemplateRevision;
  published_revision: ConsoleUiTemplateRevision | null;
  is_default: boolean;
  is_archived: boolean;
}
export interface ConsoleUiOfficialTemplate {
  provider_code: string;
  contribution_code: string;
  title: string;
  source: string;
  language: UiCodeTemplateLanguage;
  version: string;
  is_default: boolean;
}
export interface ConsoleUiTemplateList {
  official: ConsoleUiOfficialTemplate[];
  managed: ConsoleUiManagedTemplate[];
}
export interface ConsoleUiComponentUpstream {
  identity: string;
  version: string;
}
export interface ConsoleUiComponentRecord {
  id: string;
  scope_id: string;
  component_code: string;
  name: string;
  description: string;
  import_code: string;
  source_code: string;
  origin: UiComponentOrigin;
  source: string;
  group: string;
  upstream: ConsoleUiComponentUpstream;
  version: string;
  keywords: string[];
  catalog_updated_at: string | null;
  source_locator: string | null;
  source_checksum: string | null;
  created_at: string;
  updated_at: string;
}
export type CreateConsoleUiComponentInput = Omit<
  ConsoleUiComponentRecord,
  | 'id'
  | 'scope_id'
  | 'origin'
  | 'catalog_updated_at'
  | 'source_locator'
  | 'source_checksum'
  | 'created_at'
  | 'updated_at'
>;
export type UpdateConsoleUiComponentInput = Omit<
  CreateConsoleUiComponentInput,
  'component_code'
>;
export interface ConsoleUiTemplateInput {
  provider_code: string;
  contribution_code: string;
  name: string;
  source: string;
  language: UiCodeTemplateLanguage;
}

export interface ConsoleUiCatalogComponent {
  component_code: string;
  name: string;
  description: string;
  import_code: string;
  source_code: string;
  source: string;
  group: string;
  upstream: ConsoleUiComponentUpstream;
  version: string;
  keywords: string[];
  catalog_updated_at: string;
  source_locator: string;
  source_checksum: string;
}

export interface ConsoleUiCatalogIndex {
  catalog_version: string;
  generated_at: string;
  page_size: number;
  total_components: number;
  source_fingerprint: string;
}

export interface ConsoleUiCatalogPage {
  catalog_version: string;
  total_components: number;
  page_size: number;
  page: number;
  cursor: string;
  next_cursor: string | null;
  records: ConsoleUiCatalogComponent[];
}

export interface ConsoleUiCatalogSearchEntry {
  component_code: string;
  name: string;
  description: string;
  source: string;
  group: string;
  upstream: ConsoleUiComponentUpstream;
  version: string;
  keywords: string[];
  catalog_page: number;
}

export interface ConsoleUiCatalogSearchResult {
  catalog_version: string;
  page: number;
  page_size: number;
  total_entries: number;
  entries: ConsoleUiCatalogSearchEntry[];
}

export interface ConsoleUiCatalogGroupUpdate {
  source: string;
  group: string;
  remote_records: number;
  new_or_updated_records: number;
  removed_records: number;
  update_available: boolean;
}

export interface ConsoleUiCatalogUpdateStatus {
  catalog_version: string;
  source_fingerprint: string;
  update_available: boolean;
  groups: ConsoleUiCatalogGroupUpdate[];
}

const root = '/api/console/settings/ui-management';
export const fetchConsoleUiTemplates = (
  includeArchived = false,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiTemplateList>({
    path: `${root}/templates?include_archived=${includeArchived}`,
    baseUrl
  });
export const createConsoleUiTemplate = (
  input: ConsoleUiTemplateInput,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiManagedTemplate>({
    path: `${root}/templates`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
export const updateConsoleUiTemplate = (
  id: string,
  input: Pick<ConsoleUiTemplateInput, 'name' | 'source' | 'language'>,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiManagedTemplate>({
    path: `${root}/templates/${id}`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
export const publishConsoleUiTemplate = (
  id: string,
  revision: number,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiManagedTemplate>({
    path: `${root}/templates/${id}/publish`,
    method: 'POST',
    body: { revision },
    csrfToken,
    baseUrl
  });
export const setConsoleUiTemplateDefault = (
  id: string,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<void>({
    path: `${root}/templates/${id}/default`,
    method: 'PUT',
    csrfToken,
    baseUrl
  });
export const resetConsoleUiTemplateDefault = (
  locator: Pick<ConsoleUiTemplateInput, 'provider_code' | 'contribution_code'>,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<void>({
    path: `${root}/templates/default`,
    method: 'DELETE',
    body: locator,
    csrfToken,
    baseUrl
  });
export const archiveConsoleUiTemplate = (
  id: string,
  archived: boolean,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiManagedTemplate>({
    path: `${root}/templates/${id}/archive`,
    method: 'PUT',
    body: { archived },
    csrfToken,
    baseUrl
  });
export const fetchConsoleUiComponents = (baseUrl?: string) =>
  apiFetch<ConsoleUiComponentRecord[]>({
    path: `${root}/components`,
    baseUrl
  });
export const fetchConsoleUiComponent = (id: string, baseUrl?: string) =>
  apiFetch<ConsoleUiComponentRecord>({
    path: `${root}/components/${id}`,
    baseUrl
  });
export const createConsoleUiComponent = (
  input: CreateConsoleUiComponentInput,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiComponentRecord>({
    path: `${root}/components`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
export const updateConsoleUiComponent = (
  id: string,
  input: UpdateConsoleUiComponentInput,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiComponentRecord>({
    path: `${root}/components/${id}`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
export const deleteConsoleUiComponent = (
  id: string,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<void>({
    path: `${root}/components/${id}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });

export const fetchConsoleUiCatalogPage = (page: number, baseUrl?: string) =>
  apiFetch<ConsoleUiCatalogPage>({
    path: `${root}/components/catalog/pages/${page}`,
    baseUrl
  });

export const fetchConsoleUiCatalogIndex = (baseUrl?: string) =>
  apiFetch<ConsoleUiCatalogIndex>({
    path: `${root}/components/catalog/index`,
    baseUrl
  });

export const searchConsoleUiCatalog = (
  query: string,
  page = 1,
  pageSize = 20,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiCatalogSearchResult>({
    path: `${root}/components/catalog/search?q=${encodeURIComponent(query)}&page=${page}&page_size=${pageSize}`,
    baseUrl
  });

export const fetchConsoleUiCatalogUpdateStatus = (baseUrl?: string) =>
  apiFetch<ConsoleUiCatalogUpdateStatus>({
    path: `${root}/components/catalog/update-status`,
    baseUrl
  });

export const downloadConsoleUiCatalogComponent = (
  componentCode: string,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiCatalogComponent>({
    path: `${root}/components/catalog/${encodeURIComponent(componentCode)}/download`,
    method: 'POST',
    csrfToken,
    baseUrl
  });

export const syncConsoleUiCatalogGroup = (
  source: string,
  group: string,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<{ synchronized_records: number }>({
    path: `${root}/components/catalog/groups/${encodeURIComponent(source)}/${encodeURIComponent(group)}/sync`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
