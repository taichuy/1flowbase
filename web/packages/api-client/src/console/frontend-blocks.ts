import { apiFetch } from '../transport';

export interface ConsoleFrontendBlockContextContract {
  primitives: string[];
  input_schema: Record<string, unknown>;
}

export interface ConsoleFrontendBlockPermissions {
  network: string;
  storage: string;
  secrets: string;
}

export interface ConsoleFrontendContributionProvenance {
  module_id: string;
  module_version: string;
  module_kind: 'boot_core' | 'trusted_host' | 'runtime' | 'capability' | 'user';
}

export type ConsoleFrontendContributionDisableReason =
  | 'verification_invalid'
  | 'desired_state_inactive'
  | 'artifact_unavailable'
  | 'version_mismatch'
  | 'assignment_missing'
  | 'assignment_workspace_mismatch'
  | 'assignment_stale'
  | 'catalog_identity_mismatch'
  | 'unsupported_runtime'
  | 'permission_denied'
  | 'asset_invalid';

export interface ConsoleFrontendBlockCatalogEntry {
  installation_id: string;
  provider_code: string;
  plugin_id: string;
  plugin_version: string;
  contribution_code: string;
  title: string;
  runtime: string;
  entry: string;
  code_template?: string | null;
  code_template_version?: string | null;
  code_template_language?: 'jsx' | 'tsx' | null;
  isolated_entry_asset?: {
    media_type: string;
    sha256: string;
    url: string;
    integrity: 'verified_sha256';
  } | null;
  context_contract: ConsoleFrontendBlockContextContract;
  permissions: ConsoleFrontendBlockPermissions;
  ui_capabilities: string[];
  frontend_contribution_id: string;
  frontend_block_id: string;
  frontend_block_version: string;
  runtime_kind: 'trusted_native' | 'isolated';
  execution_kind: 'ui_mount';
  isolation_requirement: 'trusted_host_realm' | 'independent_realm';
  requested_permissions: string[];
  granted_permissions: string[];
  workspace_id: string;
  lifecycle_kind: 'workspace_assignment';
  graph_fingerprint: string;
  provenance: ConsoleFrontendContributionProvenance;
  disable_reason: ConsoleFrontendContributionDisableReason | null;
}

export function listConsoleFrontendBlocks(
  baseUrl?: string
): Promise<ConsoleFrontendBlockCatalogEntry[]> {
  return apiFetch<ConsoleFrontendBlockCatalogEntry[]>({
    path: '/api/console/frontend-blocks',
    method: 'GET',
    baseUrl
  });
}
