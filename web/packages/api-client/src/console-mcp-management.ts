import { apiFetch, apiFetchBlob, apiFetchVoid } from './transport';

export interface ConsoleMcpInstance {
  id: string;
  workspace_id: string;
  instance_id: string;
  name: string;
  description_short: string | null;
  status: string;
  default_entry_path: string;
  created_by: string;
  updated_by: string;
  created_at: string;
  updated_at: string;
}

export interface ConsoleMcpClientCredential {
  saved: boolean;
  api_key?: string;
}

export interface ConsoleMcpGroup {
  id: string;
  instance_record_id: string;
  path: string;
  display_name: string;
  description_short: string | null;
  enabled: boolean;
  sort_order: number;
}

export interface ConsoleMcpInterfaceWrapperExecutionTarget {
  kind: 'interface_wrapper';
  interface_id: string;
}

export interface ConsoleMcpProxyExecutionTarget {
  kind: 'mcp_proxy';
  upstream_connection_id: string;
  remote_tool_name: string;
  source_schema_hash: string;
}

export type ConsoleMcpToolExecutionTarget =
  | ConsoleMcpInterfaceWrapperExecutionTarget
  | ConsoleMcpProxyExecutionTarget;

export type ConsoleMcpToolAvailabilityStatus =
  | 'available'
  | 'interface_missing'
  | 'upstream_disabled'
  | 'credentials_missing'
  | 'upstream_tool_missing'
  | 'mapping_invalid';

export interface ConsoleMcpProxyInputMappingEntry {
  local_path: string;
  remote_path: string;
  required: boolean;
}

export interface ConsoleMcpProxyInputMapping {
  mappings: ConsoleMcpProxyInputMappingEntry[];
}

export interface ConsoleMcpProxyOutputMappingEntry {
  remote_path: string;
  local_path: string;
  required: boolean;
}

export interface ConsoleMcpProxyOutputMapping {
  mappings: ConsoleMcpProxyOutputMappingEntry[];
}

export interface ConsoleMcpTool {
  id: string;
  workspace_id: string;
  tool_id: string;
  name: string;
  short_description: string;
  full_description: string;
  execution_target: ConsoleMcpToolExecutionTarget;
  operation: string;
  parameter_schema: unknown;
  result_schema: unknown;
  input_mapping: unknown;
  output_mapping: unknown;
  permission_code: string | null;
  risk_level: string;
  des_id: string;
  des_id_required: boolean;
  status: string;
  availability_status: ConsoleMcpToolAvailabilityStatus;
  availability_reason: string | null;
  revision: number;
}

export interface ConsoleMcpBundleManifest {
  schema_version: string;
  organization: string;
  bundle_id: string;
  bundle_version: string;
  locale: 'zh_Hans' | 'en_US';
  minimum_host_version: string;
  exported_from_system_version: string;
  exported_at: string;
  files: Array<{
    path: string;
    kind: 'tool' | 'instance' | 'connection';
    sha256: string;
  }>;
}

export type ConsoleMcpBundleVersionStatus =
  | 'same_system_version'
  | 'exported_from_older_system'
  | 'exported_from_newer_system'
  | 'unknown_system_version';

export interface ConsoleMcpBundleItemReport {
  id: string;
  result: 'imported' | 'unavailable' | 'skipped' | 'failed';
  reason: string | null;
}

export interface ConsoleMcpBundlePreview {
  manifest: ConsoleMcpBundleManifest;
  current_system_version: string;
  version_status: ConsoleMcpBundleVersionStatus;
  tools: ConsoleMcpBundleItemReport[];
  instances: ConsoleMcpBundleItemReport[];
  connections: ConsoleMcpBundleItemReport[];
}

export interface ConsoleMcpBundleImportReport extends ConsoleMcpBundlePreview {
  status: 'completed' | 'completed_with_warnings' | 'failed';
}

export interface ExportConsoleMcpBundleBody {
  organization: string;
  bundle_id: string;
  bundle_version: string;
  locale: 'zh_Hans' | 'en_US';
  minimum_host_version: string;
}

export interface ConsoleMcpBundleExportDefaults {
  minimum_host_version: string;
  current_system_version: string;
}

export interface ConsoleOfficialMcpBundleEntry {
  organization: string;
  bundle_id: string;
  latest_version: string;
  locale: 'zh_Hans' | 'en_US';
  minimum_host_version: string;
  exported_from_system_version: string;
  release_tag: string;
  download_url: string;
  artifact_sha256: string | null;
}

export interface ConsoleOfficialMcpBundleCatalog {
  source: {
    source_kind: string;
    source_label: string;
    catalog_url: string;
  };
  entries: ConsoleOfficialMcpBundleEntry[];
}

export interface ConsoleOfficialMcpBundleBody {
  organization: string;
  bundle_id: string;
}

export interface ConsoleMcpToolBinding {
  id: string;
  instance_record_id: string;
  tool_record_id: string;
  group_path: string;
  tool_id: string;
  display_alias: string | null;
  visible: boolean;
  sort_order: number;
}

export interface ConsoleMcpInstanceDiscoveryPolicy {
  id: string;
  workspace_id: string;
  instance_record_id: string;
  instance_id: string;
  list_default_limit: number;
  list_max_depth: number;
  list_regex_enabled: boolean;
  list_regex_max_length: number;
  list_return_fields: unknown;
}

export type ConsoleMcpParameterType = 'url' | 'form' | 'json_body';

export interface ConsoleMcpParameterDescriptor {
  name: string;
  field_type: string;
  parameter_type: ConsoleMcpParameterType;
  description: string | null;
  required: boolean;
  schema: unknown;
}

export interface ConsoleMcpCatalog {
  instances: ConsoleMcpInstance[];
  groups: ConsoleMcpGroup[];
  tools: ConsoleMcpTool[];
  bindings: ConsoleMcpToolBinding[];
  discovery_policies: ConsoleMcpInstanceDiscoveryPolicy[];
}

export interface ConsoleMcpInterfaceCapability {
  interface_id: string;
  method: string;
  path: string;
  name: string;
  short_description: string;
  parameter_descriptors: ConsoleMcpParameterDescriptor[];
  parameter_schema: unknown;
  result_schema: unknown;
  permission_code: string | null;
  security: unknown;
  risk_level: string;
  bindable: boolean;
  disabled_reason: string | null;
}

export interface ConsoleMcpListItemSummary {
  id?: string;
  item_kind?: string;
  path?: string;
  name?: string;
  description_short?: string | null;
  children_count?: number;
  risk_level?: string | null;
}

export interface ConsoleMcpExportPackage {
  instances: ConsoleMcpInstance[];
  groups: ConsoleMcpGroup[];
  tools: ConsoleMcpTool[];
  bindings: ConsoleMcpToolBinding[];
  discovery_policies: ConsoleMcpInstanceDiscoveryPolicy[];
}

export interface SaveConsoleMcpInstanceBody {
  instance_id: string;
  name: string;
  description_short: string | null;
  status: string;
  default_entry_path: string;
}

export interface CopyConsoleMcpInstanceBody {
  instance_id: string;
  name: string;
}

export interface SaveConsoleMcpGroupBody {
  path: string;
  display_name: string;
  description_short: string | null;
  enabled: boolean;
  sort_order: number;
}

export interface MoveConsoleMcpGroupBody {
  source_path: string;
  target_parent_path: string;
  sort_order: number;
}

interface SaveConsoleMcpToolBodyBase {
  tool_id: string;
  des_id: string;
  name: string;
  short_description: string;
  full_description: string;
  parameter_schema: unknown;
  result_schema: unknown;
  permission_code: string | null;
  risk_level: string;
  status: string;
}

export type SaveConsoleMcpToolBody = SaveConsoleMcpToolBodyBase &
  (
    | {
        execution_target: ConsoleMcpInterfaceWrapperExecutionTarget;
        input_mapping: unknown;
        output_mapping: unknown;
      }
    | {
        execution_target: ConsoleMcpProxyExecutionTarget;
        input_mapping: ConsoleMcpProxyInputMapping;
        output_mapping: ConsoleMcpProxyOutputMapping;
      }
  );

type DistributiveOmit<T, K extends PropertyKey> = T extends unknown
  ? Omit<T, K>
  : never;

export type UpdateConsoleMcpToolBody = DistributiveOmit<
  SaveConsoleMcpToolBody,
  'tool_id'
>;

export interface SaveConsoleMcpToolBindingBody {
  group_path: string;
  tool_id: string;
  display_alias: string | null;
  visible: boolean;
  sort_order: number;
}

export type UpdateConsoleMcpInstanceDiscoveryPolicyBody = Omit<
  ConsoleMcpInstanceDiscoveryPolicy,
  'id' | 'workspace_id' | 'instance_record_id' | 'instance_id'
>;

export type ConsoleMcpToolDebugResponseMode = 'tool_result' | 'debug_details';

export interface ExecuteConsoleMcpToolDebugBody {
  interface_id: string;
  debug_response_mode?: ConsoleMcpToolDebugResponseMode;
  mcp_arguments: unknown;
  input_mapping: unknown;
  output_mapping: unknown;
}

export type ConsoleMcpToolDebugExecuteResponse = unknown;

export interface ConsoleMcpToolDebugDetailsResponse {
  mcp_arguments: unknown;
  interface_arguments: unknown;
  interface_response: unknown;
  tool_result: unknown;
}

export type ConsoleMcpUpstreamTransport = 'streamable_http';
export type ConsoleMcpUpstreamAuthType = 'none' | 'bearer' | 'custom_header';
export type ConsoleMcpUpstreamConnectionStatus = 'enabled' | 'disabled';
export type ConsoleMcpUpstreamCredentialsStatus =
  | 'not_required'
  | 'configured'
  | 'missing';

export interface ConsoleMcpUpstreamConnection {
  connection_id: string;
  workspace_id: string;
  name: string;
  endpoint: string;
  transport: ConsoleMcpUpstreamTransport;
  auth_type: ConsoleMcpUpstreamAuthType;
  status: ConsoleMcpUpstreamConnectionStatus;
  credentials_status: ConsoleMcpUpstreamCredentialsStatus;
  custom_header_name: string | null;
  last_connected_at: string | null;
  last_discovered_at: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface SaveConsoleMcpUpstreamConnectionBody {
  name: string;
  endpoint: string;
  transport: ConsoleMcpUpstreamTransport;
  auth_type: ConsoleMcpUpstreamAuthType;
  custom_header_name: string | null;
  status: ConsoleMcpUpstreamConnectionStatus;
}

export type SaveConsoleMcpUpstreamConnectionCredentialsBody =
  | { kind: 'bearer'; token: string }
  | {
      kind: 'custom_header';
      header_name: string;
      header_value: string;
    };

export interface ConsoleMcpUpstreamConnectionTestResult {
  connection_id: string;
  ok: boolean;
  server_name: string | null;
  server_version: string | null;
  protocol_version: string | null;
  tested_at: string;
  error: string | null;
}

export interface ConsoleMcpUpstreamConnectionDraftTestResult {
  ok: boolean;
  server_name: string | null;
  server_version: string | null;
  protocol_version: string | null;
  tested_at: string;
  error: string | null;
}

export interface TestConsoleMcpUpstreamConnectionDraftBody {
  connection_id: string | null;
  endpoint: string;
  transport: ConsoleMcpUpstreamTransport;
  auth_type: ConsoleMcpUpstreamAuthType;
  custom_header_name: string | null;
  credential: SaveConsoleMcpUpstreamConnectionCredentialsBody | null;
}

export type ConsoleMcpUpstreamToolSourceStatus =
  | 'not_imported'
  | 'imported'
  | 'definition_changed'
  | 'remote_missing';

export interface ConsoleMcpUpstreamDiscoveredTool {
  remote_tool_name: string;
  description: string | null;
  input_schema: unknown;
  output_schema: unknown;
  source_status: ConsoleMcpUpstreamToolSourceStatus;
  imported_tool_id: string | null;
  schema_hash: string;
}

export interface ConsoleMcpUpstreamDiscoveryResult {
  connection_id: string;
  server_name: string | null;
  server_version: string | null;
  protocol_version: string;
  discovered_at: string;
  items: ConsoleMcpUpstreamDiscoveredTool[];
}

export interface ImportConsoleMcpUpstreamToolsBody {
  remote_tool_names: string[];
}

export interface ExecuteConsoleMcpProxyToolDebugBody {
  arguments: unknown;
}

export interface ConsoleMcpProxyToolDebugResponse {
  local_arguments: unknown;
  remote_arguments: unknown;
  upstream_result: unknown;
  mapped_result: unknown;
}

export function fetchConsoleMcpCatalog(baseUrl?: string) {
  return apiFetch<ConsoleMcpCatalog>({
    path: '/api/console/mcp/catalog',
    baseUrl
  });
}

export function fetchConsoleMcpInterfaceCapabilities(
  options: { bindable_only?: boolean } = {},
  baseUrl?: string
) {
  const params = new URLSearchParams();
  if (options.bindable_only !== undefined) {
    params.set('bindable_only', String(options.bindable_only));
  }
  const query = params.toString();
  return apiFetch<ConsoleMcpInterfaceCapability[]>({
    path: `/api/console/mcp/interface-capabilities${query ? `?${query}` : ''}`,
    baseUrl
  });
}

export function fetchConsoleMcpListItems(
  options: {
    instance_id?: string;
    path?: string;
    path_regex?: string;
    limit?: number;
  } = {},
  baseUrl?: string
) {
  const params = new URLSearchParams();
  if (options.instance_id) {
    params.set('instance_id', options.instance_id);
  }
  if (options.path) {
    params.set('path', options.path);
  }
  if (options.path_regex) {
    params.set('path_regex', options.path_regex);
  }
  if (options.limit !== undefined) {
    params.set('limit', String(options.limit));
  }
  const query = params.toString();
  return apiFetch<ConsoleMcpListItemSummary[]>({
    path: `/api/console/mcp/list${query ? `?${query}` : ''}`,
    baseUrl
  });
}

export function exportConsoleMcpCatalog(baseUrl?: string) {
  return apiFetch<ConsoleMcpExportPackage>({
    path: '/api/console/mcp/export',
    baseUrl
  });
}

export function fetchConsoleMcpBundleExportDefaults(baseUrl?: string) {
  return apiFetch<ConsoleMcpBundleExportDefaults>({
    path: '/api/console/mcp/bundles/export-defaults',
    baseUrl
  });
}

export function exportConsoleMcpBundle(
  body: ExportConsoleMcpBundleBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchBlob({
    path: '/api/console/mcp/bundles/export',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function exportConsoleMcpInstanceBundle(
  instanceId: string,
  body: ExportConsoleMcpBundleBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchBlob({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/bundles/export`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

function uploadConsoleMcpBundle<T>(
  path: string,
  file: File,
  csrfToken: string,
  baseUrl?: string
) {
  const formData = new FormData();
  formData.set('file', file);
  return apiFetch<T>({
    path,
    method: 'POST',
    rawBody: formData,
    contentType: null,
    csrfToken,
    baseUrl
  });
}

export function previewConsoleMcpBundle(
  file: File,
  csrfToken: string,
  baseUrl?: string
) {
  return uploadConsoleMcpBundle<ConsoleMcpBundlePreview>(
    '/api/console/mcp/bundles/preview-upload',
    file,
    csrfToken,
    baseUrl
  );
}

export function importConsoleMcpBundle(
  file: File,
  csrfToken: string,
  baseUrl?: string
) {
  return uploadConsoleMcpBundle<ConsoleMcpBundleImportReport>(
    '/api/console/mcp/bundles/import-upload',
    file,
    csrfToken,
    baseUrl
  );
}

export function fetchConsoleOfficialMcpBundles(baseUrl?: string) {
  return apiFetch<ConsoleOfficialMcpBundleCatalog>({
    path: '/api/console/mcp/bundles/official',
    baseUrl
  });
}

export function previewConsoleOfficialMcpBundle(
  body: ConsoleOfficialMcpBundleBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpBundlePreview>({
    path: '/api/console/mcp/bundles/preview-official',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function importConsoleOfficialMcpBundle(
  body: ConsoleOfficialMcpBundleBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpBundleImportReport>({
    path: '/api/console/mcp/bundles/import-official',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleMcpTool(toolId: string, baseUrl?: string) {
  return apiFetch<ConsoleMcpTool>({
    path: `/api/console/mcp/tools/${encodeURIComponent(toolId)}`,
    baseUrl
  });
}

export function createConsoleMcpInstance(
  body: SaveConsoleMcpInstanceBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpInstance>({
    path: '/api/console/mcp/instances',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleMcpInstance(
  instanceId: string,
  body: SaveConsoleMcpInstanceBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpInstance>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}`,
    method: 'PUT',
    body,
    csrfToken,
    baseUrl
  });
}

export function copyConsoleMcpInstance(
  sourceInstanceId: string,
  body: CopyConsoleMcpInstanceBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpInstance>({
    path: `/api/console/mcp/instances/${encodeURIComponent(sourceInstanceId)}/copy`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpInstance(
  instanceId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleMcpClientCredential(
  instanceId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpClientCredential>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/client-credential`,
    baseUrl
  });
}

export function saveConsoleMcpClientCredential(
  instanceId: string,
  apiKey: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpClientCredential>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/client-credential`,
    method: 'PUT',
    body: { api_key: apiKey },
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpClientCredential(
  instanceId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/client-credential`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function upsertConsoleMcpGroup(
  instanceId: string,
  body: SaveConsoleMcpGroupBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpGroup>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/groups`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function moveConsoleMcpGroup(
  instanceId: string,
  body: MoveConsoleMcpGroupBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpGroup>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/groups/move`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpGroup(
  instanceId: string,
  path: string,
  csrfToken: string,
  baseUrl?: string
) {
  const params = new URLSearchParams({ path });
  return apiFetchVoid({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/groups?${params.toString()}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function createConsoleMcpTool(
  body: SaveConsoleMcpToolBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpTool>({
    path: '/api/console/mcp/tools',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleMcpTool(
  toolId: string,
  body: UpdateConsoleMcpToolBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpTool>({
    path: `/api/console/mcp/tools/${encodeURIComponent(toolId)}`,
    method: 'PUT',
    body,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpTool(
  toolId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/mcp/tools/${encodeURIComponent(toolId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function refreshConsoleMcpToolDescription(
  toolId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpTool>({
    path: `/api/console/mcp/tools/${encodeURIComponent(toolId)}/description/refresh`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function executeConsoleMcpToolDebug(
  body: ExecuteConsoleMcpToolDebugBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpToolDebugExecuteResponse>({
    path: '/api/console/mcp/debug/execute',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function createConsoleMcpToolBinding(
  instanceId: string,
  body: SaveConsoleMcpToolBindingBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpToolBinding>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/tool-bindings`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleMcpToolBinding(
  bindingId: string,
  body: SaveConsoleMcpToolBindingBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpToolBinding>({
    path: `/api/console/mcp/tool-bindings/${encodeURIComponent(bindingId)}`,
    method: 'PUT',
    body,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpToolBinding(
  bindingId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/mcp/tool-bindings/${encodeURIComponent(bindingId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function updateConsoleMcpInstanceDiscoveryPolicy(
  instanceId: string,
  body: UpdateConsoleMcpInstanceDiscoveryPolicyBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpInstanceDiscoveryPolicy>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/discovery-policy`,
    method: 'PUT',
    body,
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleMcpInstanceDiscoveryPolicy(
  instanceId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpInstanceDiscoveryPolicy>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/discovery-policy`,
    baseUrl
  });
}

export function fetchConsoleMcpUpstreamConnections(baseUrl?: string) {
  return apiFetch<ConsoleMcpUpstreamConnection[]>({
    path: '/api/console/mcp/upstream-connections',
    baseUrl
  });
}

export function createConsoleMcpUpstreamConnection(
  body: SaveConsoleMcpUpstreamConnectionBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpUpstreamConnection>({
    path: '/api/console/mcp/upstream-connections',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleMcpUpstreamConnection(
  connectionId: string,
  body: SaveConsoleMcpUpstreamConnectionBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpUpstreamConnection>({
    path: `/api/console/mcp/upstream-connections/${encodeURIComponent(connectionId)}`,
    method: 'PUT',
    body,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpUpstreamConnection(
  connectionId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/mcp/upstream-connections/${encodeURIComponent(connectionId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function saveConsoleMcpUpstreamConnectionCredentials(
  connectionId: string,
  body: SaveConsoleMcpUpstreamConnectionCredentialsBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/mcp/upstream-connections/${encodeURIComponent(connectionId)}/credentials`,
    method: 'PUT',
    body,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpUpstreamConnectionCredentials(
  connectionId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/mcp/upstream-connections/${encodeURIComponent(connectionId)}/credentials`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function testConsoleMcpUpstreamConnection(
  connectionId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpUpstreamConnectionTestResult>({
    path: `/api/console/mcp/upstream-connections/${encodeURIComponent(connectionId)}/test`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function testConsoleMcpUpstreamConnectionDraft(
  body: TestConsoleMcpUpstreamConnectionDraftBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpUpstreamConnectionDraftTestResult>({
    path: '/api/console/mcp/upstream-connections/test',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function discoverConsoleMcpUpstreamConnection(
  connectionId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpUpstreamDiscoveryResult>({
    path: `/api/console/mcp/upstream-connections/${encodeURIComponent(connectionId)}/discover`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function importConsoleMcpUpstreamTools(
  connectionId: string,
  body: ImportConsoleMcpUpstreamToolsBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpTool[]>({
    path: `/api/console/mcp/upstream-connections/${encodeURIComponent(connectionId)}/imports`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function executeConsoleMcpProxyToolDebug(
  toolId: string,
  body: ExecuteConsoleMcpProxyToolDebugBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpProxyToolDebugResponse>({
    path: `/api/console/mcp/tools/${encodeURIComponent(toolId)}/debug`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}
