import { apiFetch } from '../../transport';

// The server owns package schema validation. Preserve the complete parsed JSON.
export type PortableTemplatePackage = unknown;
export interface PortableTemplateSelection {
  page_ids: string[];
  application_ids: string[];
  data_model_ids: string[];
  mcp_instance_ids: string[];
}
export interface PortableCatalogItem {
  id: string;
  name: string;
  parent_id: string | null;
  code: string | null;
}
export interface ApplicationTemplateCatalogItem {
  template_id: string;
  release_version: number;
  name: string;
  description: string;
  checksum: string;
  installed_release_version: number | null;
  installed_checksum: string | null;
  catalog_id: string;
  source: 'builtin' | 'official';
}
export interface ApplicationTemplateCatalogPage {
  application_templates: ApplicationTemplateCatalogItem[];
  next_cursor: string | null;
  total: number;
}
export interface ApplicationTemplateCatalogQuery {
  cursor?: string;
  q?: string;
}
export interface ApplicationTemplateReference {
  catalog_id: string;
  release_version: number;
}
export interface PortableTemplateArchive {
  archive_base64: string;
}
export interface PortableTemplateArchiveExport extends PortableTemplateArchive {
  file_name: string;
}
export interface PortableTemplateCatalog {
  pages: PortableCatalogItem[];
  applications: PortableCatalogItem[];
  data_models: PortableCatalogItem[];
  mcp_instances: { id: string; name: string }[];
}
export interface PortableTemplatePreview {
  valid: boolean;
  counts: {
    pages: number;
    applications: number;
    data_models: number;
    mcp_instances: number;
  };
  failures: string[];
  warnings: string[];
  effects: {
    kind: string;
    source_id: string;
    target_id: string | null;
    action: string;
  }[];
  mcp_shared_tool_impacts: { tool_id: string; instance_ids: string[] }[];
  dependencies: {
    plugin_id: string;
    plugin_version: string;
    checksum: string | null;
    contribution_code: string | null;
  }[];
}
export interface PortableTemplateInstallResult {
  complete: boolean;
  created: { kind: string; source_id: string; target_id: string }[];
  updated: { kind: string; source_id: string; target_id: string }[];
  id_map: Record<string, string>;
  failures: string[];
}
const BASE_PATH = '/api/console/settings/system-templates';
export const getSystemTemplateCatalog = (baseUrl?: string) =>
  apiFetch<PortableTemplateCatalog>({ path: `${BASE_PATH}/catalog`, baseUrl });
export const getApplicationTemplateCatalog = (
  query: ApplicationTemplateCatalogQuery = {},
  baseUrl?: string
) => {
  const search = new URLSearchParams({ category: 'applications-demo' });
  if (query.cursor) search.set('cursor', query.cursor);
  if (query.q) search.set('q', query.q);
  return apiFetch<ApplicationTemplateCatalogPage>({
    path: `${BASE_PATH}/catalog?${search}`,
    baseUrl
  });
};
export const exportSystemTemplateArchive = (
  body: PortableTemplateSelection,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<PortableTemplateArchiveExport>({
    path: `${BASE_PATH}/export?format=archive`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
export const exportSystemTemplate = (
  body: PortableTemplateSelection,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<PortableTemplatePackage>({
    path: `${BASE_PATH}/export`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
export const previewSystemTemplate = (
  body: PortableTemplatePackage,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<PortableTemplatePreview>({
    path: `${BASE_PATH}/preview`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
export const installSystemTemplate = (
  body: PortableTemplatePackage,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<PortableTemplateInstallResult>({
    path: `${BASE_PATH}/install`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
