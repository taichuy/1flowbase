import { apiFetch } from './transport';

export type ConsoleApplicationNodeSourceKind = 'builtin' | 'plugin';
export type ConsoleApplicationNodeCategory =
  | 'io'
  | 'generation'
  | 'control'
  | 'data'
  | 'external';
export type ConsoleApplicationNodeRuntimeStatus = 'ready' | 'unavailable';
export type ConsoleApplicationNodeDependencyStatus =
  | 'not_applicable'
  | 'ready'
  | 'missing_plugin'
  | 'version_mismatch'
  | 'disabled_plugin';

export interface ConsoleApplicationNodeContractField {
  key: string;
  description: string;
  required: boolean;
  value_types: string[];
  allowed_values: string[];
  applicability: string | null;
}

export interface ConsoleApplicationNodeFieldContract {
  config_fields: ConsoleApplicationNodeContractField[];
  input_fields: ConsoleApplicationNodeContractField[];
  output_fields: ConsoleApplicationNodeContractField[];
}

export interface ConsolePluginNodeIdentity {
  installation_id: string;
  provider_code: string;
  plugin_unique_identifier: string;
  package_id: string;
  plugin_id: string;
  plugin_version: string;
  contribution_code: string;
  node_shell: string;
  category: string;
  title: string;
  description: string;
  schema_version: string;
  experimental: boolean;
  icon: string;
  schema_ui: Record<string, unknown>;
  output_schema: Record<string, unknown>;
  contribution_checksum: string;
  compiled_contribution_hash: string;
  output_schema_snapshot: Record<string, unknown>;
  side_effect_policy: string;
  infra_contracts: string[];
  required_auth: string[];
  visibility: string;
  dependency_installation_kind: string;
  dependency_plugin_version_range: string;
}

export interface ConsoleApplicationNodeCatalogEntry {
  source_kind: ConsoleApplicationNodeSourceKind;
  node_type: string;
  title: string;
  description: string;
  category: ConsoleApplicationNodeCategory;
  runtime_status: ConsoleApplicationNodeRuntimeStatus;
  runtime_status_description: string;
  dependency_status: ConsoleApplicationNodeDependencyStatus;
  field_contract: ConsoleApplicationNodeFieldContract;
  plugin: ConsolePluginNodeIdentity | null;
}

export interface ConsoleApplicationNodeCatalog {
  nodes: ConsoleApplicationNodeCatalogEntry[];
}

function buildNodeContributionsPath(applicationId: string) {
  const params = new URLSearchParams({
    application_id: applicationId
  });
  return `/api/console/node-contributions?${params.toString()}`;
}

export function listConsoleNodeContributions(
  applicationId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleApplicationNodeCatalog>({
    path: buildNodeContributionsPath(applicationId),
    baseUrl
  });
}
