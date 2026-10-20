export interface ConsoleMcpInstance {
  id: string;
  workspace_id: string;
  instance_id: string;
  name: string;
  description_short: string | null;
  status: string;
  default_entry_path: string;
  webmcp_exposure: WebMcpExposure;
  managed_by: ConsoleMcpManagedBundleSource | null;
  created_by: string;
  updated_by: string;
  created_at: string;
  updated_at: string;
  llm_tool_registration: ConsoleMcpLlmToolRegistration;
}

export type WebMcpExposure = 'disabled' | 'authenticated_session';

export interface ConsoleMcpManagedBundleSource {
  organization: string;
  bundle_id: string;
  bundle_version: string;
}

export interface ConsoleMcpLlmToolRegistration {
  prefix: string;
  tools: Array<{
    operation: 'list' | 'get' | 'result' | 'call';
    name: string;
  }>;
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

export interface ConsoleMcpAssistantClientExecutionTarget {
  kind: 'assistant_client';
  capability_code: string;
}

export type ConsoleMcpToolExecutionTarget =
  | ConsoleMcpInterfaceWrapperExecutionTarget
  | ConsoleMcpProxyExecutionTarget
  | ConsoleMcpAssistantClientExecutionTarget;

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
  max_inline_chars: number | null;
  response_fields: string[] | null;
  permission_code: string | null;
  risk_level: string;
  des_id: string;
  des_id_required: boolean;
  status: string;
  availability_status: ConsoleMcpToolAvailabilityStatus;
  availability_reason: string | null;
  revision: number;
  managed_by: ConsoleMcpManagedBundleSource | null;
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

export type ConsoleMcpBundleItemEffect =
  | 'create'
  | 'update'
  | 'already_present'
  | 'conflict'
  | 'failed';

export interface ConsoleMcpBundleEffectSummary {
  changes: number;
  already_present: number;
  conflicts: number;
  unavailable: number;
  failed: number;
}

export interface ConsoleMcpBundleItemReport {
  id: string;
  effect: ConsoleMcpBundleItemEffect;
  result: 'imported' | 'already_present' | 'unavailable' | 'skipped' | 'failed';
  reason: string | null;
}

export interface ConsoleMcpBundlePreview {
  manifest: ConsoleMcpBundleManifest;
  current_system_version: string;
  version_status: ConsoleMcpBundleVersionStatus;
  effect_summary: ConsoleMcpBundleEffectSummary;
  tools: ConsoleMcpBundleItemReport[];
  instances: ConsoleMcpBundleItemReport[];
  connections: ConsoleMcpBundleItemReport[];
  shared_tool_impacts: Array<{
    tool_id: string;
    instance_ids: string[];
  }>;
}

export interface ConsoleMcpBundleImportReport extends ConsoleMcpBundlePreview {
  status:
    | 'completed'
    | 'completed_with_warnings'
    | 'already_applied'
    | 'failed';
}

export interface ExportConsoleMcpBundleBody {
  organization: string;
  bundle_id: string;
  bundle_version: string;
  locale: 'zh_Hans' | 'en_US';
}

export interface ConsoleMcpBundleExportDefaults {
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

export interface ConsoleMcpTemplateLibraryVersion {
  bundle_version: string;
  locale: 'zh_Hans' | 'en_US';
  minimum_host_version: string;
  exported_from_system_version: string;
  checksum: string;
  algorithm?: string;
  key_id?: string;
  signature?: string;
  signature_status?: string;
  downloaded_at?: string;
}

export interface ConsoleMcpTemplateLibraryBundle {
  organization: string;
  bundle_id: string;
  current_bundle_version: string | null;
  remote_versions: ConsoleMcpTemplateLibraryVersion[];
  local_versions: ConsoleMcpTemplateLibraryVersion[];
}

export interface ConsoleMcpTemplateLibrary {
  source: {
    source_kind: string;
    source_label: string;
    catalog_url: string;
  };
  remote_available: boolean;
  remote_error: string | null;
  bundles: ConsoleMcpTemplateLibraryBundle[];
}

export interface ConsoleMcpTemplateLibraryVersionBody {
  bundle_version?: string;
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
  webmcp_exposure: WebMcpExposure;
}

export interface CopyConsoleMcpInstanceBody {
  instance_id: string;
  name: string;
}

export interface SaveConsoleMcpGroupBody {
  path: string;
  display_name?: string | null;
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
  max_inline_chars?: number | null;
  response_fields?: string[] | null;
  tool_id: string;
  des_id: string;
  name: string;
  short_description: string;
  full_description?: string;
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
    | {
        execution_target: ConsoleMcpAssistantClientExecutionTarget;
        input_mapping: unknown;
        output_mapping: unknown;
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
