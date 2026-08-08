import { apiFetch, apiFetchVoid } from '../transport';

export interface ConsoleSessionActor {
  id: string;
  account: string;
  effective_display_role: string;
  current_workspace_id: string;
}

export interface ConsoleSessionRecord {
  id: string;
  user_id: string;
  tenant_id: string;
  current_workspace_id: string;
  active_role_code: string;
}

export interface ConsoleAvailableRole {
  code: string;
  name: string;
  scope_kind: 'system' | 'workspace';
}

export interface ConsoleSessionSnapshot {
  actor: ConsoleSessionActor;
  session: ConsoleSessionRecord;
  available_roles: ConsoleAvailableRole[];
  active_role_permissions: string[];
  csrf_token: string;
  cookie_name: string;
}

export function switchConsoleSessionRole(
  roleCode: string,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleSessionSnapshot> {
  return apiFetch<ConsoleSessionSnapshot>({
    path: '/api/console/session/actions/switch-role',
    method: 'POST',
    body: { role_code: roleCode },
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleSession(
  baseUrl?: string
): Promise<ConsoleSessionSnapshot> {
  return apiFetch<ConsoleSessionSnapshot>({
    path: '/api/console/session',
    baseUrl
  });
}

export function deleteConsoleSession(
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: '/api/console/session',
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}
