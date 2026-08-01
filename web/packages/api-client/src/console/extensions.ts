import { ApiClientError } from '../errors';
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
  message: string;
  overridable: boolean;
}

export interface ConsoleExtensionCompatibilityChallenge {
  reason: 'below_minimum_host_version';
  current_host_version: string;
  minimum_host_version: string;
}

export interface ConsoleExtensionRiskChallenge {
  warnings: ConsoleExtensionWarning[];
  compatibility: ConsoleExtensionCompatibilityChallenge | null;
}

export interface ConsoleInstalledExtension {
  id: string;
  category: ConsoleExtensionCategory;
  organization: string;
  artifact_id: string;
  version: string;
  node_id: string;
  source: string;
  trust: string;
  warnings: ConsoleExtensionWarning[];
  local_path: string;
  checksum: string;
  signature_status: string;
  signature_algorithm: string | null;
  signing_key_id: string | null;
  status: string;
  installed_by: string;
  created_at: string;
  updated_at: string;
}

export interface ConsoleInstalledExtensionPage {
  limit: number;
  next_cursor: string | null;
  entries: ConsoleInstalledExtension[];
}

export interface ConsoleExtensionCatalogEntry {
  category: ConsoleExtensionCategory;
  id: string;
  name: string;
  organization: string;
  artifact: string;
  version: string;
  description: string;
  host_version_requirement: string;
  source: Record<string, unknown>;
  signature: Record<string, unknown> | null;
  checksum: string | null;
  download_locator: Record<string, unknown>;
  catalog_page: number;
  catalog_source: string;
  current_version: string | null;
  installation_status: string;
  artifact_kind: string | null;
  installation_source: string | null;
  trust: string;
  warnings: ConsoleExtensionWarning[];
  compatibility: ConsoleExtensionCompatibilityChallenge | null;
}

export interface ConsoleExtensionCatalogPage {
  category: ConsoleExtensionCategory;
  catalog_page: string;
  catalog_page_number: number;
  catalog_page_checksum: string;
  catalog_page_locator: string;
  limit: number;
  next_cursor: string | null;
  total_entries: number;
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

export interface ConsoleExtensionInstallResponse {
  installation: ConsoleInstalledExtension;
  local_artifact_was_present: boolean;
  node_plugin_installation_id: string | null;
}

export interface ConsoleExtensionUploadMetadata {
  category: ConsoleExtensionCategory;
  organization?: string;
  artifact_id?: string;
  version?: string;
}

interface ConsoleExtensionRiskChallengeErrorBody {
  status: number;
  code: 'extension_risk_confirmation_required';
  message: string;
  risk_challenge: ConsoleExtensionRiskChallenge;
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

export function getConsoleExtensionCatalogEntry(
  category: ConsoleExtensionCategory,
  artifactId: string
) {
  return apiFetch<ConsoleExtensionCatalogEntry>({
    path: `${BASE}/catalog/${category}/${encodeURIComponent(artifactId)}`
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
    compatibility_override?: ConsoleExtensionCompatibilityOverride;
    risk_override?: ConsoleExtensionRiskOverride;
  },
  csrfToken: string,
  update = false
) {
  return apiFetch<ConsoleExtensionInstallResponse>({
    path: `${BASE}/${update ? 'update' : 'install'}`,
    method: 'POST',
    body: input,
    csrfToken
  });
}

export function uploadConsoleExtension(
  file: File,
  metadata: ConsoleExtensionUploadMetadata,
  csrfToken: string,
  overrides: {
    compatibility_override?: ConsoleExtensionCompatibilityOverride;
    risk_override?: ConsoleExtensionRiskOverride;
  } = {}
) {
  const formData = new FormData();
  formData.append('file', file);
  formData.append('category', metadata.category);
  if (metadata.organization) {
    formData.append('organization', metadata.organization);
  }
  if (metadata.artifact_id) {
    formData.append('artifact_id', metadata.artifact_id);
  }
  if (metadata.version) {
    formData.append('version', metadata.version);
  }
  if (overrides.compatibility_override) {
    formData.append(
      'compatibility_override',
      JSON.stringify(overrides.compatibility_override)
    );
  }
  if (overrides.risk_override) {
    formData.append('risk_override', JSON.stringify(overrides.risk_override));
  }

  return apiFetch<ConsoleExtensionInstallResponse>({
    path: `${BASE}/install-upload`,
    method: 'POST',
    rawBody: formData,
    contentType: null,
    csrfToken
  });
}

export function getConsoleExtensionRiskChallenge(
  error: unknown
): ConsoleExtensionRiskChallenge | null {
  if (!(error instanceof ApiClientError) || error.status !== 409) return null;
  if (!error.body || typeof error.body !== 'object') return null;

  const body = error.body as Partial<ConsoleExtensionRiskChallengeErrorBody>;
  return body.code === 'extension_risk_confirmation_required' &&
    body.risk_challenge &&
    Array.isArray(body.risk_challenge.warnings)
    ? body.risk_challenge
    : null;
}
