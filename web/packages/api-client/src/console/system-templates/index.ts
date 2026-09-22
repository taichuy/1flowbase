import { apiFetch } from '../../transport';

// The server owns package schema validation. Preserve the complete parsed JSON.
export type PortableTemplatePackage = unknown;
export interface PortableTemplateSelection {
  page_ids: string[];
  application_ids: string[];
  data_model_ids: string[];
}
export interface PortableCatalogItem {
  id: string;
  name: string;
  parent_id: string | null;
  code: string | null;
}
export interface PortableTemplateCatalog {
  pages: PortableCatalogItem[];
  applications: PortableCatalogItem[];
  data_models: PortableCatalogItem[];
}
export interface PortableTemplatePreview {
  valid: boolean;
  counts: { pages: number; applications: number; data_models: number };
  failures: string[];
  warnings: string[];
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
  id_map: Record<string, string>;
  failures: string[];
}
const BASE_PATH = '/api/console/settings/system-templates';
export const getSystemTemplateCatalog = (baseUrl?: string) =>
  apiFetch<PortableTemplateCatalog>({ path: `${BASE_PATH}/catalog`, baseUrl });
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
