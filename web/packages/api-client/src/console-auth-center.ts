import { apiFetch } from './transport';

export interface ConsoleAuthCenterConfigField {
  key: string;
  label: string;
  type: string;
  control?: string;
  read_only?: boolean;
  required?: boolean;
  pattern?: string;
}

export interface ConsoleAuthCenterLoginEntryConfigValues {
  title: string;
  enabled: boolean;
  description?: string | null;
  extension_config?: Record<string, unknown>;
  [key: string]: unknown;
}

export interface ConsoleAuthCenterContextVariable {
  group: 'configuration' | 'runtime';
  label: string;
  member_path: string;
  schema: Record<string, unknown>;
}

export interface ConsoleAuthCenterLoginEntry {
  id: string;
  auth_type: string;
  title: string;
  enabled: boolean;
  is_builtin: boolean;
  sort_order: number;
  public_ui_block: string;
  default_public_ui_block?: string;
  interface_path_prefixes: string[];
  public_variables: Record<string, unknown> | null;
  context_variables: ConsoleAuthCenterContextVariable[];
  config_schema: ConsoleAuthCenterConfigField[];
  config_values: ConsoleAuthCenterLoginEntryConfigValues;
}

export interface ConsoleAuthCenterLoginEntryConfigInput {
  title: string;
  enabled: boolean;
  description?: string | null;
  self_registration_enabled: boolean;
  extension_config: Record<string, unknown>;
}

export interface ConsoleAuthCenterLoginEntryPublicUiBlockInput {
  public_ui_block: string;
}

export interface ConsoleAuthCenterLoginEntryEnabledInput {
  enabled: boolean;
}

export interface ConsoleAuthCenterOverview {
  default_login_entry_id: string;
  supported_auth_types: string[];
  login_entries: ConsoleAuthCenterLoginEntry[];
}

export interface ConsoleAuthCenterCreateLoginEntryInput {
  auth_type: string;
  title: string;
  description?: string | null;
  enabled: boolean;
  sort_order?: number;
}

export interface ConsoleAuthCenterCopyLoginEntryInput {
  title: string;
  sort_order?: number;
}

export interface ConsoleAuthCenterReorderLoginEntriesInput {
  ids: string[];
}

export function fetchConsoleAuthCenterOverview(baseUrl?: string) {
  return apiFetch<ConsoleAuthCenterOverview>({
    path: '/api/console/settings/auth-center/overview',
    baseUrl
  });
}

export function updateConsoleAuthCenterLoginEntryEnabled(
  loginEntryId: string,
  input: ConsoleAuthCenterLoginEntryEnabledInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleAuthCenterLoginEntry> {
  return apiFetch<ConsoleAuthCenterLoginEntry>({
    path: `/api/console/settings/auth-center/login-entries/${encodeURIComponent(loginEntryId)}/enabled`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function createConsoleAuthCenterLoginEntry(
  input: ConsoleAuthCenterCreateLoginEntryInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleAuthCenterLoginEntry> {
  return apiFetch<ConsoleAuthCenterLoginEntry>({
    path: '/api/console/settings/auth-center/login-entries',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function copyConsoleAuthCenterLoginEntry(
  sourceId: string,
  input: ConsoleAuthCenterCopyLoginEntryInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleAuthCenterLoginEntry> {
  return apiFetch<ConsoleAuthCenterLoginEntry>({
    path: `/api/console/settings/auth-center/login-entries/${encodeURIComponent(sourceId)}/copy`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleAuthCenterLoginEntry(
  loginEntryId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetch<void>({
    path: `/api/console/settings/auth-center/login-entries/${encodeURIComponent(loginEntryId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl,
    expectJson: false
  });
}

export function reorderConsoleAuthCenterLoginEntries(
  input: ConsoleAuthCenterReorderLoginEntriesInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleAuthCenterOverview> {
  return apiFetch<ConsoleAuthCenterOverview>({
    path: '/api/console/settings/auth-center/login-entries/order',
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleAuthCenterLoginEntryConfig(
  loginEntryId: string,
  input: ConsoleAuthCenterLoginEntryConfigInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleAuthCenterLoginEntry> {
  return apiFetch<ConsoleAuthCenterLoginEntry>({
    path: `/api/console/settings/auth-center/login-entries/${encodeURIComponent(loginEntryId)}/config`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleAuthCenterLoginEntryPublicUiBlock(
  loginEntryId: string,
  input: ConsoleAuthCenterLoginEntryPublicUiBlockInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleAuthCenterLoginEntry> {
  return apiFetch<ConsoleAuthCenterLoginEntry>({
    path: `/api/console/settings/auth-center/login-entries/${encodeURIComponent(loginEntryId)}/public-ui-block`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}
