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

export interface ConsoleAuthCenterAuthenticatorConfigValues {
  title: string;
  enabled: boolean;
  description?: string | null;
  extension_config?: Record<string, unknown>;
  [key: string]: unknown;
}

export interface ConsoleAuthCenterAuthenticator {
  id: string;
  auth_type: string;
  title: string;
  enabled: boolean;
  is_builtin: boolean;
  sort_order: number;
  config_schema: ConsoleAuthCenterConfigField[];
  config_values: ConsoleAuthCenterAuthenticatorConfigValues;
}

export interface ConsoleAuthCenterAuthenticatorConfigInput {
  title: string;
  enabled: boolean;
  description?: string | null;
  self_registration_enabled: boolean;
  public_ui_block: string;
  extension_config: Record<string, unknown>;
}

export interface ConsoleAuthCenterOverview {
  default_authenticator_id: string;
  supported_auth_types: string[];
  authenticators: ConsoleAuthCenterAuthenticator[];
}

export interface ConsoleAuthCenterCreateAuthenticatorInput {
  auth_type: string;
  title: string;
  description?: string | null;
  enabled: boolean;
  sort_order?: number;
}

export interface ConsoleAuthCenterCopyAuthenticatorInput {
  title: string;
  sort_order?: number;
}

export interface ConsoleAuthCenterReorderAuthenticatorsInput {
  ids: string[];
}

export function fetchConsoleAuthCenterOverview(baseUrl?: string) {
  return apiFetch<ConsoleAuthCenterOverview>({
    path: '/api/console/settings/auth-center/overview',
    baseUrl
  });
}

export function enableConsoleAuthCenterAuthenticator(
  authenticatorId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleAuthCenterAuthenticator> {
  return apiFetch<ConsoleAuthCenterAuthenticator>({
    path: `/api/console/settings/auth-center/authenticators/${encodeURIComponent(authenticatorId)}/actions/enable`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function createConsoleAuthCenterAuthenticator(
  input: ConsoleAuthCenterCreateAuthenticatorInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleAuthCenterAuthenticator> {
  return apiFetch<ConsoleAuthCenterAuthenticator>({
    path: '/api/console/settings/auth-center/authenticators',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function copyConsoleAuthCenterAuthenticator(
  sourceId: string,
  input: ConsoleAuthCenterCopyAuthenticatorInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleAuthCenterAuthenticator> {
  return apiFetch<ConsoleAuthCenterAuthenticator>({
    path: `/api/console/settings/auth-center/authenticators/${encodeURIComponent(sourceId)}/copy`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleAuthCenterAuthenticator(
  authenticatorId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetch<void>({
    path: `/api/console/settings/auth-center/authenticators/${encodeURIComponent(authenticatorId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl,
    expectJson: false
  });
}

export function reorderConsoleAuthCenterAuthenticators(
  input: ConsoleAuthCenterReorderAuthenticatorsInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleAuthCenterOverview> {
  return apiFetch<ConsoleAuthCenterOverview>({
    path: '/api/console/settings/auth-center/authenticators/order',
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleAuthCenterAuthenticatorConfig(
  authenticatorId: string,
  input: ConsoleAuthCenterAuthenticatorConfigInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleAuthCenterAuthenticator> {
  return apiFetch<ConsoleAuthCenterAuthenticator>({
    path: `/api/console/settings/auth-center/authenticators/${encodeURIComponent(authenticatorId)}/config`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}
