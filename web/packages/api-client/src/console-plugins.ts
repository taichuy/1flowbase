import { apiFetch } from './transport';
import type { ConsolePluginFormFieldSchema } from './console-model-providers';

export interface ConsolePluginCatalogFilter {
  plugin_type?: string;
  locale?: string;
  q?: string;
  cursor?: string;
  limit?: number;
}

export interface ConsolePluginArtifactInstance {
  node_id: string;
  installation_id: string;
  local_version: string | null;
  local_checksum: string | null;
  installed_path: string | null;
  artifact_status: string;
  runtime_status: string;
  checked_at: string;
  last_error: string | null;
}

export interface ConsolePluginInstallation {
  id: string;
  provider_code: string;
  plugin_id: string;
  plugin_version: string;
  contract_version: string;
  protocol: string;
  display_name: string;
  source_kind: string;
  trust_level: string;
  verification_status: string;
  desired_state: string;
  artifact_status: string;
  runtime_status: string;
  availability_status: string;
  package_path?: string | null;
  installed_path?: string;
  checksum: string | null;
  manifest_fingerprint?: string | null;
  signature_status: string | null;
  signature_algorithm: string | null;
  signing_key_id: string | null;
  last_load_error: string | null;
  local_artifact?: ConsolePluginArtifactInstance | null;
  metadata_json: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface ConsolePluginCatalogEntry {
  installation: ConsolePluginInstallation;
  local_artifact: ConsolePluginArtifactInstance;
  plugin_type: string;
  namespace: string;
  label_key: string;
  description_key: string | null;
  provider_label_key: string;
  help_url: string | null;
  default_base_url: string | null;
  model_discovery_mode: string;
  assigned_to_current_workspace: boolean;
}

export interface ConsolePluginCatalogResponse {
  locale_meta: Record<string, unknown>;
  i18n_catalog: Record<string, unknown>;
  entries: ConsolePluginCatalogEntry[];
}

export type ConsoleOfficialPluginInstallStatus =
  | 'not_installed'
  | 'installed'
  | 'assigned';

export type ConsoleOfficialPluginCompatibilityStatus =
  | 'compatible'
  | 'below_minimum_host_version';

export interface ConsolePluginCompatibilityOverride {
  reason: 'below_minimum_host_version';
  acknowledged_current_host_version: string;
  acknowledged_minimum_host_version: string;
}

export interface ConsoleOfficialPluginArtifact {
  os: string;
  arch: string;
  libc: string | null;
  rust_target: string;
  download_url: string;
  checksum: string;
  signature_algorithm: string | null;
  signing_key_id: string | null;
}

export interface ConsoleOfficialPluginCatalogEntry {
  plugin_id: string;
  provider_code: string;
  plugin_type: string;
  display_name: string;
  description: string | null;
  icon?: string | null;
  protocol: string;
  latest_version: string;
  minimum_host_version: string;
  current_host_version: string;
  compatibility_status: ConsoleOfficialPluginCompatibilityStatus;
  compatibility_warning_reason: string | null;
  selected_artifact: ConsoleOfficialPluginArtifact;
  help_url: string | null;
  model_discovery_mode: string;
  install_status: ConsoleOfficialPluginInstallStatus;
}

export interface ConsoleOfficialPluginCatalogPage {
  limit: number;
  next_cursor: string | null;
}

export interface ConsoleOfficialPluginCatalogResponse {
  source_kind: string;
  source_label: string;
  registry_url: string;
  source_freshness: 'fresh' | 'stale';
  locale_meta: Record<string, unknown>;
  page: ConsoleOfficialPluginCatalogPage;
  entries: ConsoleOfficialPluginCatalogEntry[];
}

export interface ConsolePluginInstalledVersion {
  installation_id: string;
  plugin_version: string;
  source_kind: string;
  trust_level: string;
  desired_state: string;
  availability_status: string;
  local_artifact: ConsolePluginArtifactInstance;
  created_at: string;
  is_current: boolean;
}

export interface ConsolePluginFamilyEntry {
  provider_code: string;
  plugin_type: string;
  namespace: string;
  label_key: string;
  description_key: string | null;
  provider_label_key: string;
  icon?: string | null;
  protocol: string;
  help_url: string | null;
  default_base_url: string | null;
  model_discovery_mode: string;
  current_installation_id: string;
  current_version: string;
  current_local_artifact: ConsolePluginArtifactInstance;
  latest_version: string | null;
  has_update: boolean;
  installed_versions: ConsolePluginInstalledVersion[];
}

export interface ConsolePluginFamilyCatalogResponse {
  locale_meta: Record<string, unknown>;
  i18n_catalog: Record<string, unknown>;
  entries: ConsolePluginFamilyEntry[];
}

export interface ConsolePluginTask {
  id: string;
  installation_id: string | null;
  workspace_id: string | null;
  provider_code: string;
  task_kind: string;
  status: string;
  status_message: string | null;
  detail_json: Record<string, unknown>;
  created_at: string;
  updated_at: string;
  finished_at: string | null;
}

export interface InstallConsolePluginInput {
  package_root: string;
}

export interface InstallConsoleOfficialPluginInput {
  plugin_id: string;
  compatibility_override?: ConsolePluginCompatibilityOverride;
}

export interface UpgradeConsolePluginFamilyLatestInput {
  compatibility_override?: ConsolePluginCompatibilityOverride;
}

export interface InstallConsolePluginResult {
  installation: ConsolePluginInstallation;
  task: ConsolePluginTask;
}

export interface ConsoleHostInfrastructureProviderConfig {
  installation_id: string;
  extension_id: string;
  provider_code: string;
  display_name: string;
  description: string | null;
  runtime_status: string;
  desired_state: string;
  config_ref: string;
  contracts: string[];
  enabled_contracts: string[];
  config_schema: ConsolePluginFormFieldSchema[];
  config_json: Record<string, unknown>;
  restart_required: boolean;
}

export interface SaveConsoleHostInfrastructureProviderConfigInput {
  enabled_contracts: string[];
  config_json: Record<string, unknown>;
}

export interface ConsoleCacheInspectionCapabilities {
  list_domains: boolean;
  list_entries: boolean;
  reveal_value: boolean;
  clear_entry: boolean;
  clear_domain: boolean;
}

export interface ConsoleCacheDomain {
  domain_code: string;
  entry_count: number;
  total_value_size_bytes: number;
}

export interface ConsoleCacheEntryMetadata {
  domain_code: string;
  key: string;
  value_size_bytes: number;
  ttl_seconds: number | null;
  created_at_unix: number | null;
  expires_at_unix: number | null;
}

export interface ConsoleHostInfrastructureCacheOverview {
  provider_code: string | null;
  can_manage: boolean;
  capabilities: ConsoleCacheInspectionCapabilities;
  domains: ConsoleCacheDomain[];
}

export interface ConsoleHostInfrastructureCacheEntries {
  domain_code: string;
  capabilities: ConsoleCacheInspectionCapabilities;
  entries: ConsoleCacheEntryMetadata[];
}

export interface ConsoleCacheEntryValue {
  metadata: ConsoleCacheEntryMetadata;
  value: unknown;
}

export interface ClearConsoleCacheEntryResult {
  cleared: boolean;
}

export interface ClearConsoleCacheDomainResult {
  cleared_count: number;
}

export interface ConsoleMemoryObservationCapabilities {
  list_entries: boolean;
  list_tree: boolean;
  search_entries: boolean;
  reveal_value: boolean;
  default_page_size: number;
  max_page_size: number;
  default_byte_limit: number;
  max_byte_limit: number;
  default_preview_size_bytes: number;
  max_full_value_size_bytes: number;
  max_value_size_bytes: number;
  max_payload_size_bytes: number;
}

export interface ConsoleMemoryContractSummary {
  contract_code: string;
  label: string;
  provider_code: string | null;
  capabilities: ConsoleMemoryObservationCapabilities;
  supported: boolean;
}

export interface ConsoleMemoryEntryMetadata {
  contract_code: string;
  group_code: string | null;
  entry_ref: string;
  key: string;
  inspection_path: string[];
  entry_kind: string;
  status: string;
  owner: string | null;
  value_size_bytes: number;
  metadata_size_bytes: number;
  ttl_seconds: number | null;
  created_at_unix: number | null;
  expires_at_unix: number | null;
  sensitive: boolean;
  metadata: Record<string, unknown>;
}

export interface ConsoleHostInfrastructureMemoryOverview {
  can_manage: boolean;
  contracts: ConsoleMemoryContractSummary[];
}

export interface ConsoleMemoryStats {
  contract_code: string;
  label: string;
  provider_code: string | null;
  capabilities: ConsoleMemoryObservationCapabilities;
  supported: boolean;
  inspection_path: string[];
  entry_count: number;
  sensitive_entry_count: number;
  total_value_size_bytes: number;
}

export interface ConsoleMemoryStatsOverview {
  inspection_path: string[];
  contracts: ConsoleMemoryStats[];
  entry_count: number;
  sensitive_entry_count: number;
  total_value_size_bytes: number;
}

export interface ConsoleHostInfrastructureMemoryEntries {
  contract_code: string;
  label: string;
  provider_code: string | null;
  capabilities: ConsoleMemoryObservationCapabilities;
  supported: boolean;
  inspection_path: string[];
  entries: ConsoleMemoryEntryMetadata[];
  next_cursor: string | null;
  limit: number;
  byte_limit: number;
  emitted_bytes: number;
  truncated_by_byte_limit: boolean;
}

export interface ConsoleMemoryTreeNode {
  node_ref: string;
  label: string;
  inspection_path: string[];
  depth: number;
  has_children: boolean;
}

export interface ConsoleHostInfrastructureMemoryTree {
  contract_code: string;
  label: string;
  provider_code: string | null;
  capabilities: ConsoleMemoryObservationCapabilities;
  supported: boolean;
  inspection_path: string[];
  nodes: ConsoleMemoryTreeNode[];
  next_cursor: string | null;
  limit: number;
  byte_limit: number;
  emitted_bytes: number;
  truncated_by_byte_limit: boolean;
}

export type ConsoleMemoryRevealMode = 'metadata' | 'preview' | 'full';
export type ConsoleMemoryValueState =
  | 'hidden'
  | 'available'
  | 'preview'
  | 'value_too_large';

export interface ConsoleMemoryEntryValue {
  metadata: ConsoleMemoryEntryMetadata;
  reveal_mode: ConsoleMemoryRevealMode;
  value_state: ConsoleMemoryValueState;
  value: unknown | null;
  value_preview: string | null;
  preview_size_bytes: number;
  full_value_size_bytes: number;
}

export interface ConsoleMemoryPageRequest {
  inspection_path?: string[];
  cursor?: string | null;
  limit?: number;
  byte_limit?: number;
}

export interface ConsoleMemorySearchRequest extends ConsoleMemoryPageRequest {
  q: string;
}

function buildPluginCatalogPath(
  path: string,
  filter?: ConsolePluginCatalogFilter
) {
  if (!filter || Object.keys(filter).length === 0) {
    return path;
  }

  const params = new URLSearchParams();

  if (filter.plugin_type) {
    params.set('plugin_type', filter.plugin_type);
  }

  if (filter.locale) {
    params.set('locale', filter.locale);
  }

  if (filter.q) {
    params.set('q', filter.q);
  }

  if (filter.cursor) {
    params.set('cursor', filter.cursor);
  }

  if (filter.limit !== undefined) {
    params.set('limit', String(filter.limit));
  }

  const queryString = params.toString();
  return queryString ? `${path}?${queryString}` : path;
}

function buildMemoryInspectionPath(
  path: string,
  request?: ConsoleMemoryPageRequest
) {
  if (!request) {
    return path;
  }
  const params = new URLSearchParams();
  if (request.inspection_path?.length) {
    params.set('path', request.inspection_path.join('/'));
  }
  if (request.cursor) {
    params.set('cursor', request.cursor);
  }
  if (request.limit != null) {
    params.set('limit', String(request.limit));
  }
  if (request.byte_limit != null) {
    params.set('byte_limit', String(request.byte_limit));
  }
  const queryString = params.toString();
  return queryString ? `${path}?${queryString}` : path;
}

function buildMemorySearchPath(
  path: string,
  request: ConsoleMemorySearchRequest
) {
  const basePath = buildMemoryInspectionPath(path, request);
  const separator = basePath.includes('?') ? '&' : '?';
  return `${basePath}${separator}q=${encodeURIComponent(request.q)}`;
}

export function listConsolePluginCatalog(
  filter?: ConsolePluginCatalogFilter,
  baseUrl?: string
) {
  return apiFetch<ConsolePluginCatalogResponse>({
    path: buildPluginCatalogPath('/api/console/plugins/catalog', filter),
    baseUrl
  });
}

export function listConsolePluginFamilies(
  filter?: ConsolePluginCatalogFilter,
  baseUrl?: string
) {
  return apiFetch<ConsolePluginFamilyCatalogResponse>({
    path: buildPluginCatalogPath('/api/console/plugins/families', filter),
    baseUrl
  });
}

export function listConsoleOfficialPluginCatalog(
  filter?: ConsolePluginCatalogFilter,
  baseUrl?: string
) {
  return apiFetch<ConsoleOfficialPluginCatalogResponse>({
    path: buildPluginCatalogPath(
      '/api/console/plugins/official-catalog',
      filter
    ),
    baseUrl
  });
}

export function installConsolePlugin(
  input: InstallConsolePluginInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<InstallConsolePluginResult>({
    path: '/api/console/plugins/install',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function installConsoleOfficialPlugin(
  input: InstallConsoleOfficialPluginInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<InstallConsolePluginResult>({
    path: '/api/console/plugins/install-official',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function uploadConsolePluginPackage(
  file: File,
  csrfToken: string,
  baseUrl?: string
) {
  const formData = new FormData();
  formData.set('file', file);

  return apiFetch<InstallConsolePluginResult>({
    path: '/api/console/plugins/install-upload',
    method: 'POST',
    rawBody: formData,
    contentType: null,
    csrfToken,
    baseUrl
  });
}

export function enableConsolePlugin(
  installationId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsolePluginTask>({
    path: `/api/console/plugins/${installationId}/enable`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function assignConsolePlugin(
  installationId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsolePluginTask>({
    path: `/api/console/plugins/${installationId}/assign`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function refreshConsolePluginCurrentNodeArtifact(
  installationId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsolePluginArtifactInstance>({
    path: `/api/console/plugins/${installationId}/artifact/refresh`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function installConsolePluginCurrentNodeArtifact(
  installationId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsolePluginArtifactInstance>({
    path: `/api/console/plugins/${installationId}/artifact/install-current-node`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function upgradeConsolePluginFamilyLatest(
  providerCode: string,
  csrfToken: string,
  inputOrBaseUrl?: UpgradeConsolePluginFamilyLatestInput | string,
  baseUrl?: string
) {
  const input = typeof inputOrBaseUrl === 'string' ? undefined : inputOrBaseUrl;
  const resolvedBaseUrl =
    typeof inputOrBaseUrl === 'string' ? inputOrBaseUrl : baseUrl;

  return apiFetch<ConsolePluginTask>({
    path: `/api/console/plugins/families/${providerCode}/upgrade-latest`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl: resolvedBaseUrl
  });
}

export function switchConsolePluginFamilyVersion(
  providerCode: string,
  input: { installation_id: string },
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsolePluginTask>({
    path: `/api/console/plugins/families/${providerCode}/switch-version`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsolePluginFamily(
  providerCode: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsolePluginTask>({
    path: `/api/console/plugins/families/${providerCode}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function listConsolePluginTasks(baseUrl?: string) {
  return apiFetch<ConsolePluginTask[]>({
    path: '/api/console/plugins/tasks',
    baseUrl
  });
}

export function getConsolePluginTask(taskId: string, baseUrl?: string) {
  return apiFetch<ConsolePluginTask>({
    path: `/api/console/plugins/tasks/${taskId}`,
    baseUrl
  });
}

export function listConsoleHostInfrastructureProviders(baseUrl?: string) {
  return apiFetch<ConsoleHostInfrastructureProviderConfig[]>({
    path: '/api/console/settings/host-infrastructure/providers',
    baseUrl
  });
}

export function getConsoleHostInfrastructureCacheOverview(baseUrl?: string) {
  return apiFetch<ConsoleHostInfrastructureCacheOverview>({
    path: '/api/console/settings/host-infrastructure/cache',
    baseUrl
  });
}

export function listConsoleHostInfrastructureCacheEntries(
  domainCode: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleHostInfrastructureCacheEntries>({
    path: `/api/console/settings/host-infrastructure/cache/domains/${encodeURIComponent(
      domainCode
    )}/entries`,
    baseUrl
  });
}

export function revealConsoleHostInfrastructureCacheEntry(
  domainCode: string,
  key: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleCacheEntryValue>({
    path: `/api/console/settings/host-infrastructure/cache/domains/${encodeURIComponent(
      domainCode
    )}/entries/reveal`,
    method: 'POST',
    body: { key },
    csrfToken,
    baseUrl
  });
}

export function clearConsoleHostInfrastructureCacheEntry(
  domainCode: string,
  key: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ClearConsoleCacheEntryResult>({
    path: `/api/console/settings/host-infrastructure/cache/domains/${encodeURIComponent(
      domainCode
    )}/entries/clear`,
    method: 'POST',
    body: { key },
    csrfToken,
    baseUrl
  });
}

export function clearConsoleHostInfrastructureCacheDomain(
  domainCode: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ClearConsoleCacheDomainResult>({
    path: `/api/console/settings/host-infrastructure/cache/domains/${encodeURIComponent(
      domainCode
    )}/clear`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function getConsoleHostInfrastructureMemoryOverview(baseUrl?: string) {
  return apiFetch<ConsoleHostInfrastructureMemoryOverview>({
    path: '/api/console/settings/host-infrastructure/memory',
    baseUrl
  });
}

export function getConsoleHostInfrastructureMemoryStatsOverview(
  baseUrl?: string
) {
  return apiFetch<ConsoleMemoryStatsOverview>({
    path: '/api/console/settings/host-infrastructure/memory/stats',
    baseUrl
  });
}

export function getConsoleHostInfrastructureMemoryStats(
  contractCode: string,
  request?: ConsoleMemoryPageRequest,
  baseUrl?: string
) {
  return apiFetch<ConsoleMemoryStats>({
    path: buildMemoryInspectionPath(
      `/api/console/settings/host-infrastructure/memory/contracts/${encodeURIComponent(
        contractCode
      )}/stats`,
      request
    ),
    baseUrl
  });
}

export function listConsoleHostInfrastructureMemoryEntries(
  contractCode: string,
  request?: ConsoleMemoryPageRequest,
  baseUrl?: string
) {
  return apiFetch<ConsoleHostInfrastructureMemoryEntries>({
    path: buildMemoryInspectionPath(
      `/api/console/settings/host-infrastructure/memory/contracts/${encodeURIComponent(
        contractCode
      )}/entries`,
      request
    ),
    baseUrl
  });
}

export function listConsoleHostInfrastructureMemoryTree(
  contractCode: string,
  request?: ConsoleMemoryPageRequest,
  baseUrl?: string
) {
  return apiFetch<ConsoleHostInfrastructureMemoryTree>({
    path: buildMemoryInspectionPath(
      `/api/console/settings/host-infrastructure/memory/contracts/${encodeURIComponent(
        contractCode
      )}/tree`,
      request
    ),
    baseUrl
  });
}

export function searchConsoleHostInfrastructureMemoryEntries(
  contractCode: string,
  request: ConsoleMemorySearchRequest,
  baseUrl?: string
) {
  return apiFetch<ConsoleHostInfrastructureMemoryEntries>({
    path: buildMemorySearchPath(
      `/api/console/settings/host-infrastructure/memory/contracts/${encodeURIComponent(
        contractCode
      )}/entries/search`,
      request
    ),
    baseUrl
  });
}

export function revealConsoleHostInfrastructureMemoryEntry(
  contractCode: string,
  entryRef: string,
  csrfToken: string,
  revealMode: ConsoleMemoryRevealMode = 'preview',
  baseUrl?: string
) {
  return apiFetch<ConsoleMemoryEntryValue>({
    path: `/api/console/settings/host-infrastructure/memory/contracts/${encodeURIComponent(
      contractCode
    )}/entries/reveal`,
    method: 'POST',
    body: { entry_ref: entryRef, reveal_mode: revealMode },
    csrfToken,
    baseUrl
  });
}

export function saveConsoleHostInfrastructureProviderConfig(
  installationId: string,
  providerCode: string,
  input: SaveConsoleHostInfrastructureProviderConfigInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<{
    restart_required: boolean;
    installation_desired_state: string;
    provider_config_status: string;
  }>({
    path: `/api/console/settings/host-infrastructure/providers/${installationId}/${providerCode}/config`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}
