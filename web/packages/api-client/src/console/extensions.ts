import { apiFetch } from '../transport';

export type ConsoleExtensionCategory =
  | 'agent-flow'
  | 'capability-plugins'
  | 'host-extensions'
  | 'i18n'
  | 'mcp'
  | 'runtime-extensions';

export interface ConsoleExtensionWarning {
  code: string;
  overridable: boolean;
}

export interface ConsoleInstalledExtension {
  category: ConsoleExtensionCategory;
  artifact_kind: string | null;
  artifact_id: string;
  display_name: string;
  description: string | null;
  current_version: string;
  system_requirements: string | null;
  installation_status: string;
  source: string;
  trust: string;
  warnings: ConsoleExtensionWarning[];
  installation: { id: string };
  local_artifact: { installed_path: string | null };
}

export interface ConsoleInstalledExtensionPage {
  limit: number;
  next_cursor: string | null;
  entries: ConsoleInstalledExtension[];
}

export interface ConsoleExtensionCatalogEntry {
  category: ConsoleExtensionCategory;
  artifact_id: string;
  organization: string;
  display_name: string;
  description: string | null;
  latest_version: string;
  current_version: string | null;
  current_host_version: string | null;
  minimum_host_version: string | null;
  system_requirements: string | null;
  installation_status: string;
  artifact_kind: string | null;
  source: string;
  trust: string;
  warnings: ConsoleExtensionWarning[];
}

export interface ConsoleExtensionCatalogPage {
  category: ConsoleExtensionCategory;
  catalog_page: string | null;
  limit: number;
  next_cursor: string | null;
  entries: ConsoleExtensionCatalogEntry[];
}

export interface ConsoleExtensionUpdateItem {
  artifact_id: string;
  current_version: string;
  latest_version: string | null;
  status: 'current' | 'update_available' | 'unknown_error';
}

export interface ConsoleExtensionUpdateResponse {
  category: ConsoleExtensionCategory;
  catalog_page: string | null;
  items: ConsoleExtensionUpdateItem[];
}

export interface ConsoleExtensionRiskOverride {
  reason: string;
  acknowledged_warnings: string[];
}

export interface ConsoleExtensionCompatibilityOverride {
  reason: 'below_minimum_host_version';
  acknowledged_current_host_version: string;
  acknowledged_minimum_host_version: string;
}

const BASE = '/api/console/settings/extension-center';

export function listConsoleInstalledExtensions(cursor?: string, limit = 20) {
  const query = new URLSearchParams({ limit: String(limit) });
  if (cursor) query.set('cursor', cursor);
  return apiFetch<ConsoleInstalledExtensionPage>({
    path: `${BASE}/installed?${query.toString()}`
  });
}

export function listConsoleExtensionCatalog(
  category: ConsoleExtensionCategory,
  cursor?: string,
  limit = 20
) {
  const query = new URLSearchParams({ limit: String(limit) });
  if (cursor) query.set('cursor', cursor);
  return apiFetch<ConsoleExtensionCatalogPage>({
    path: `${BASE}/catalog/${category}?${query.toString()}`
  });
}

export function checkConsoleExtensionUpdates(
  input: {
    category: ConsoleExtensionCategory;
    catalog_page: string | null;
    items: Array<{ artifact_id: string; current_version: string }>;
  },
  csrfToken: string
) {
  return apiFetch<ConsoleExtensionUpdateResponse>({
    path: `${BASE}/update-check`,
    method: 'POST',
    body: input,
    csrfToken
  });
}

export function installConsoleExtension(
  input: {
    category: ConsoleExtensionCategory;
    artifact_id: string;
    artifact_kind: string | null;
    compatibility_override?: ConsoleExtensionCompatibilityOverride;
    risk_override?: ConsoleExtensionRiskOverride;
  },
  csrfToken: string,
  update = false
) {
  return apiFetch<unknown>({
    path: `${BASE}/${update ? 'update' : 'install'}`,
    method: 'POST',
    body: input,
    csrfToken
  });
}
