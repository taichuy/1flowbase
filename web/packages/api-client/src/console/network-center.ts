import { apiFetch, apiFetchVoid } from '../transport';
import type {
  InstallConsoleOfficialPluginInput,
  InstallConsolePluginResult
} from '../console-plugins';

export interface ConsoleNetworkEgressOfficialPluginCatalogEntry {
  plugin_id: string;
  provider_code: string;
  plugin_type: string;
  display_name: string;
  description: string | null;
  icon?: string | null;
  protocol: string;
  current_version: string | null;
  latest_version: string;
  has_update: boolean;
  minimum_host_version: string;
  current_host_version: string;
  compatibility_status: string;
  compatibility_warning_reason: string | null;
  selected_artifact: Record<string, unknown>;
  help_url: string | null;
  model_discovery_mode: string;
  install_status: 'not_installed' | 'installed';
}

export interface ConsoleNetworkEgressOfficialPluginCatalogResponse {
  source_kind: string;
  source_label: string;
  registry_url: string;
  source_freshness: 'fresh' | 'stale';
  locale_meta: Record<string, unknown>;
  page: { limit: number; next_cursor: string | null };
  entries: ConsoleNetworkEgressOfficialPluginCatalogEntry[];
}

export interface ConsoleNetworkEgressPluginInstalledVersion {
  installation_id: string;
  plugin_version: string;
  is_current: boolean;
  can_uninstall: boolean;
}

export interface ConsoleNetworkEgressPluginFamily {
  provider_code: string;
  display_name: string;
  current_installation_id: string;
  current_version: string;
  can_uninstall: boolean;
  installed_versions: ConsoleNetworkEgressPluginInstalledVersion[];
}

export interface ConsoleNetworkEgressProjection {
  provider_egress_key: string;
  display_name: string;
  region: string | null;
  tags: string[];
  availability: string;
  synced_at: string;
}

export interface ConsoleNetworkEgressProvider {
  id: string;
  installation_id: string | null;
  provider_code: string;
  display_name: string;
  description: string;
  lifecycle: string;
  health_status: string;
  secret_configured: boolean;
  last_sync_error: string | null;
  last_synced_at: string | null;
  egresses: ConsoleNetworkEgressProjection[];
}

export interface CreateConsoleNetworkEgressProviderInput {
  installation_id: string;
  display_name: string;
  description: string;
  config: Record<string, string>;
}

export interface ConsoleNetworkEgressProviderType {
  installation_id: string | null;
  provider_code: string;
  display_name: string;
  form_schema: {
    schema_version: string;
    fields: Array<{
      key: string;
      label: string;
      type: string;
      control?: string;
      required?: boolean;
      description?: string;
      placeholder?: string;
      send_mode?: string;
    }>;
  };
}

export interface ConsoleNetworkEgressOfficialPluginCatalogFilter {
  locale?: string;
  q?: string;
  cursor?: string;
  limit?: number;
}

function networkEgressPluginCatalogPath(
  path: string,
  filter?: ConsoleNetworkEgressOfficialPluginCatalogFilter
) {
  const params = new URLSearchParams();
  if (filter?.locale) params.set('locale', filter.locale);
  if (filter?.q) params.set('q', filter.q);
  if (filter?.cursor) params.set('cursor', filter.cursor);
  if (filter?.limit) params.set('limit', String(filter.limit));
  const query = params.toString();
  return query ? `${path}?${query}` : path;
}

export interface CreateConsoleNetworkEgressProxyInput {
  provider_code: string;
  display_name: string;
  description: string;
  config: Record<string, string>;
}

export interface UpdateConsoleNetworkEgressProviderLifecycleInput {
  lifecycle: string;
}

export interface ConsoleNetworkEgressPoolMember {
  id: string;
  provider_id: string;
  provider_egress_key: string;
  enabled: boolean;
  sequence: number;
  health: string;
  provider_code: string;
  display_name: string;
  address_summary: string | null;
  region: string | null;
  probe_status: string;
  probe_http_status: string;
  probe_https_status: string;
  probe_latency_ms: number;
  probe_exit_ip: string | null;
  probe_exit_region: string | null;
  probe_error_code: string | null;
  last_probed_at: string | null;
}

export interface ConsoleNetworkEgressPool {
  id: string;
  display_name: string;
  owner_provider_id?: string | null;
  selection_strategy: string;
  members: ConsoleNetworkEgressPoolMember[];
}

export interface CreateConsoleNetworkEgressPoolInput {
  display_name: string;
}

export interface UpdateConsoleNetworkEgressPoolInput {
  display_name: string;
}

export interface CreateConsoleNetworkEgressPoolMemberInput {
  provider_id: string;
  provider_egress_key: string;
  enabled: boolean;
  sequence: number;
}

export interface CreateConsoleNetworkEgressPoolStaticHttpMemberInput {
  display_name: string;
  host: string;
  port: number;
  username: string;
  password: string;
  enabled: boolean;
  sequence: number;
}

export interface AddConsoleNetworkEgressProviderToPoolInput {
  provider_id: string;
  enabled: boolean;
  sequence: number;
}

export interface UpdateConsoleNetworkEgressPoolMemberInput {
  enabled: boolean;
  sequence: number;
}

export interface ConsoleNetworkEgressRoute {
  id: string;
  consumer_kind: string;
  consumer_reference: string | null;
  pool_member_ids: string[];
  enabled: boolean;
  failure_policy: string;
}

export interface CreateConsoleNetworkEgressRouteInput {
  consumer_kind: string;
  consumer_reference: string | null;
  pool_member_ids: string[];
  enabled: boolean;
}

export interface UpdateConsoleNetworkEgressRouteInput {
  pool_member_ids: string[];
  enabled: boolean;
}

export function listConsoleNetworkEgressProviders(baseUrl?: string) {
  return apiFetch<ConsoleNetworkEgressProvider[]>({
    path: '/api/console/settings/network-center/providers',
    baseUrl
  });
}

export function listConsoleNetworkEgressProviderTypes(baseUrl?: string) {
  return apiFetch<ConsoleNetworkEgressProviderType[]>({
    path: '/api/console/settings/network-center/providers/types',
    baseUrl
  });
}

export function listConsoleNetworkEgressOfficialPluginCatalog(
  filter?: ConsoleNetworkEgressOfficialPluginCatalogFilter,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressOfficialPluginCatalogResponse>({
    path: networkEgressPluginCatalogPath(
      '/api/console/settings/network-center/proxy-plugins/official-catalog',
      filter
    ),
    baseUrl
  });
}

export function installConsoleNetworkEgressOfficialPlugin(
  input: InstallConsoleOfficialPluginInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<InstallConsolePluginResult>({
    path: '/api/console/settings/network-center/proxy-plugins/install-official',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function listConsoleNetworkEgressPluginFamilies(baseUrl?: string) {
  return apiFetch<ConsoleNetworkEgressPluginFamily[]>({
    path: '/api/console/settings/network-center/proxy-plugins/families',
    baseUrl
  });
}

export function switchConsoleNetworkEgressPluginVersion(
  providerCode: string,
  installationId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/settings/network-center/proxy-plugins/families/${encodeURIComponent(providerCode)}/switch-version`,
    method: 'POST',
    body: { installation_id: installationId },
    csrfToken,
    baseUrl
  });
}

export function uninstallConsoleNetworkEgressPluginVersion(
  providerCode: string,
  installationId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/settings/network-center/proxy-plugins/families/${encodeURIComponent(providerCode)}/versions/${encodeURIComponent(installationId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function uninstallConsoleNetworkEgressPluginFamily(
  providerCode: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/settings/network-center/proxy-plugins/families/${encodeURIComponent(providerCode)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function uploadConsoleNetworkEgressPluginPackage(
  file: File,
  csrfToken: string,
  baseUrl?: string
) {
  const formData = new FormData();
  formData.set('file', file);
  return apiFetch<InstallConsolePluginResult>({
    path: '/api/console/settings/network-center/proxy-plugins/install-upload',
    method: 'POST',
    rawBody: formData,
    contentType: null,
    csrfToken,
    baseUrl
  });
}

export function createConsoleNetworkEgressProvider(
  input: CreateConsoleNetworkEgressProviderInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressProvider>({
    path: '/api/console/settings/network-center/providers',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleNetworkEgressProviderLifecycle(
  providerId: string,
  input: UpdateConsoleNetworkEgressProviderLifecycleInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressProvider>({
    path: `/api/console/settings/network-center/providers/${encodeURIComponent(providerId)}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function syncConsoleNetworkEgressProvider(
  providerId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressProvider>({
    path: `/api/console/settings/network-center/providers/${encodeURIComponent(providerId)}/sync`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function listConsoleNetworkEgressPools(baseUrl?: string) {
  return apiFetch<ConsoleNetworkEgressPool[]>({
    path: '/api/console/network-center/pools',
    baseUrl
  });
}

export function createConsoleNetworkEgressProxy(
  input: CreateConsoleNetworkEgressProxyInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressProvider>({
    path: '/api/console/network-center/pools/proxies',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function createConsoleNetworkEgressPool(
  input: CreateConsoleNetworkEgressPoolInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressPool>({
    path: '/api/console/network-center/pools',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleNetworkEgressPool(
  poolId: string,
  input: UpdateConsoleNetworkEgressPoolInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressPool>({
    path: `/api/console/network-center/pools/${encodeURIComponent(poolId)}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleNetworkEgressPool(
  poolId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/network-center/pools/${encodeURIComponent(poolId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function createConsoleNetworkEgressPoolMember(
  poolId: string,
  input: CreateConsoleNetworkEgressPoolMemberInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressPoolMember>({
    path: `/api/console/network-center/pools/${encodeURIComponent(poolId)}/members`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function testConsoleNetworkEgressPoolMember(
  poolId: string,
  memberId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressPoolMember>({
    path: `/api/console/network-center/pools/${encodeURIComponent(poolId)}/members/${encodeURIComponent(memberId)}/test-connection`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function createConsoleNetworkEgressPoolStaticHttpMember(
  poolId: string,
  input: CreateConsoleNetworkEgressPoolStaticHttpMemberInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressPoolMember>({
    path: `/api/console/network-center/pools/${encodeURIComponent(poolId)}/members/static-http`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function addConsoleNetworkEgressProviderToPool(
  poolId: string,
  input: AddConsoleNetworkEgressProviderToPoolInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressPoolMember[]>({
    path: `/api/console/network-center/pools/${encodeURIComponent(poolId)}/members/provider`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleNetworkEgressPoolMember(
  poolId: string,
  memberId: string,
  input: UpdateConsoleNetworkEgressPoolMemberInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressPoolMember>({
    path: `/api/console/network-center/pools/${encodeURIComponent(poolId)}/members/${encodeURIComponent(memberId)}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleNetworkEgressPoolMember(
  poolId: string,
  memberId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/network-center/pools/${encodeURIComponent(poolId)}/members/${encodeURIComponent(memberId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function listConsoleNetworkEgressRoutes(baseUrl?: string) {
  return apiFetch<ConsoleNetworkEgressRoute[]>({
    path: '/api/console/network-center/routes',
    baseUrl
  });
}

export function createConsoleNetworkEgressRoute(
  input: CreateConsoleNetworkEgressRouteInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressRoute>({
    path: '/api/console/network-center/routes',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleNetworkEgressRoute(
  routeId: string,
  input: UpdateConsoleNetworkEgressRouteInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressRoute>({
    path: `/api/console/network-center/routes/${encodeURIComponent(routeId)}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleNetworkEgressRoute(
  routeId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/network-center/routes/${encodeURIComponent(routeId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}
