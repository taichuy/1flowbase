import { apiFetch, apiFetchVoid } from '../transport/json';

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
  policy_fingerprint: string;
  csrf_token: string;
  cookie_name: string;
}

export interface ConsoleWorkspaceSummary {
  id: string;
  name: string;
  logo_url: string | null;
  introduction: string;
  is_current: boolean;
}

export function fetchConsoleWorkspaces(
  baseUrl?: string
): Promise<ConsoleWorkspaceSummary[]> {
  return apiFetch<ConsoleWorkspaceSummary[]>({
    path: '/api/console/workspaces',
    baseUrl
  });
}

export function switchConsoleSessionWorkspace(
  workspaceId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleSessionSnapshot> {
  return apiFetch<ConsoleSessionSnapshot>({
    path: '/api/console/session/actions/switch-workspace',
    method: 'POST',
    body: { workspace_id: workspaceId },
    csrfToken,
    baseUrl
  });
}

export function redeemConsoleWorkspaceInvitation(
  invitationToken: string,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleSessionSnapshot> {
  return apiFetch<ConsoleSessionSnapshot>({
    path: '/api/console/workspaces/invitations/redeem',
    method: 'POST',
    body: { invitation_token: invitationToken },
    csrfToken,
    baseUrl
  });
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
