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
  name: string;
  title: string;
  enabled: boolean;
  description?: string | null;
  extension_config?: Record<string, unknown>;
  [key: string]: unknown;
}

export interface ConsoleAuthCenterAuthenticator {
  name: string;
  auth_type: string;
  title: string;
  enabled: boolean;
  is_builtin: boolean;
  sort_order: number;
  config_schema: ConsoleAuthCenterConfigField[];
  config_values: ConsoleAuthCenterAuthenticatorConfigValues;
}

export interface ConsoleAuthCenterAuthenticatorConfigInput {
  name?: string;
  title: string;
  enabled: boolean;
  description?: string | null;
}

export interface ConsoleAuthCenterOverview {
  default_authenticator_name: string;
  supported_auth_types: string[];
  authenticators: ConsoleAuthCenterAuthenticator[];
}

export interface ConsoleAuthCenterCreateAuthenticatorInput {
  name: string;
  auth_type: string;
  title: string;
  description?: string | null;
  enabled: boolean;
  sort_order?: number;
}

export interface ConsoleAuthCenterCopyAuthenticatorInput {
  name: string;
  title: string;
  sort_order?: number;
}

export interface ConsoleAuthCenterReorderAuthenticatorsInput {
  names: string[];
}

export function fetchConsoleAuthCenterOverview(baseUrl?: string) {
  return apiFetch<ConsoleAuthCenterOverview>({
    path: '/api/console/settings/auth-center/overview',
    baseUrl
  });
}

export function enableConsoleAuthCenterAuthenticator(
  authenticatorName: string,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleAuthCenterAuthenticator> {
  return apiFetch<ConsoleAuthCenterAuthenticator>({
    path: `/api/console/settings/auth-center/authenticators/${encodeURIComponent(authenticatorName)}/actions/enable`,
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
  authenticatorName: string,
  input: ConsoleAuthCenterCopyAuthenticatorInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleAuthCenterAuthenticator> {
  return apiFetch<ConsoleAuthCenterAuthenticator>({
    path: `/api/console/settings/auth-center/authenticators/${encodeURIComponent(authenticatorName)}/copy`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleAuthCenterAuthenticator(
  authenticatorName: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetch<void>({
    path: `/api/console/settings/auth-center/authenticators/${encodeURIComponent(authenticatorName)}`,
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
  authenticatorName: string,
  input: ConsoleAuthCenterAuthenticatorConfigInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleAuthCenterAuthenticator> {
  return apiFetch<ConsoleAuthCenterAuthenticator>({
    path: `/api/console/settings/auth-center/authenticators/${encodeURIComponent(authenticatorName)}/config`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}
