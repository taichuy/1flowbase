import { apiFetch } from './transport';

export interface ConsoleAuthCenterConfigField {
  key: string;
  label: string;
  type: string;
}

export interface ConsoleAuthCenterAuthenticator {
  name: string;
  auth_type: string;
  title: string;
  enabled: boolean;
  is_builtin: boolean;
  config_schema: ConsoleAuthCenterConfigField[];
  config_values: Record<string, unknown>;
}

export interface ConsoleAuthCenterOverview {
  default_authenticator_name: string;
  authenticators: ConsoleAuthCenterAuthenticator[];
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
