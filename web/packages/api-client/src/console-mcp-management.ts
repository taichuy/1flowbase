import { apiFetch, apiFetchVoid } from './transport';

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

export interface ConsoleMcpGroup {
  id: string;
  instance_record_id: string;
  path: string;
  display_name: string;
  description_short: string | null;
  enabled: boolean;
  sort_order: number;
}

export interface ConsoleMcpTool {
  id: string;
  workspace_id: string;
  tool_id: string;
  name: string;
  short_description: string;
  full_description: string;
  interface_id: string;
  parameter_schema: unknown;
  result_schema: unknown;
  input_mapping: unknown;
  output_mapping: unknown;
  permission_code: string | null;
  risk_level: string;
  des_id: string;
  des_id_required: boolean;
  status: string;
  revision: number;
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

export interface ConsoleMcpMetaToolConfig {
  id: string;
  workspace_id: string;
  list_default_limit: number;
  list_max_depth: number;
  list_regex_enabled: boolean;
  list_regex_max_length: number;
  list_return_fields: unknown;
  get_include_mapping_summary: boolean;
  get_include_interface_summary: boolean;
  call_default_des_id_policy: string;
  call_high_risk_requires_des_id: boolean;
  call_validation_error_format: string;
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
  meta_tool_config: ConsoleMcpMetaToolConfig;
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
  meta_tool_config: ConsoleMcpMetaToolConfig;
}

export interface ConsoleMcpInstanceDirectoryExportPackage {
  instances: ConsoleMcpInstance[];
  groups: ConsoleMcpGroup[];
  bindings: ConsoleMcpToolBinding[];
  meta_tool_config: ConsoleMcpMetaToolConfig;
}

export interface SaveConsoleMcpInstanceBody {
  instance_id: string;
  name: string;
  description_short: string | null;
  status: string;
  default_entry_path: string;
}

export interface SaveConsoleMcpGroupBody {
  path: string;
  display_name: string;
  description_short: string | null;
  enabled: boolean;
  sort_order: number;
}

export interface SaveConsoleMcpToolBody {
  tool_id: string;
  des_id: string;
  name: string;
  short_description: string;
  full_description: string;
  interface_id: string;
  parameter_schema: unknown;
  result_schema: unknown;
  input_mapping: unknown;
  output_mapping: unknown;
  permission_code: string | null;
  risk_level: string;
  status: string;
}

export type UpdateConsoleMcpToolBody = Omit<SaveConsoleMcpToolBody, 'tool_id'>;

export interface SaveConsoleMcpToolBindingBody {
  group_path: string;
  tool_id: string;
  display_alias: string | null;
  visible: boolean;
  sort_order: number;
}

export type UpdateConsoleMcpMetaToolConfigBody = Omit<
  ConsoleMcpMetaToolConfig,
  'id' | 'workspace_id'
>;

export interface ExecuteConsoleMcpToolDebugBody {
  interface_id: string;
  mcp_arguments: unknown;
  input_mapping: unknown;
  output_mapping: unknown;
}

export interface ConsoleMcpToolDebugExecuteResponse {
  mcp_arguments: unknown;
  interface_arguments: unknown;
  interface_response: unknown;
  tool_result: unknown;
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

export function exportConsoleMcpInstanceDirectory(baseUrl?: string) {
  return apiFetch<ConsoleMcpInstanceDirectoryExportPackage>({
    path: '/api/console/mcp/instances/export',
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

export function updateConsoleMcpMetaToolConfig(
  body: UpdateConsoleMcpMetaToolConfigBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpMetaToolConfig>({
    path: '/api/console/mcp/meta-tool-config',
    method: 'PUT',
    body,
    csrfToken,
    baseUrl
  });
}
