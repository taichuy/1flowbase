import {
  deleteConsoleSession as requestDeleteConsoleSession,
  fetchConsoleWorkspaces as requestFetchConsoleWorkspaces,
  redeemConsoleWorkspaceInvitation as requestRedeemConsoleWorkspaceInvitation,
  switchConsoleSessionRole as requestSwitchConsoleSessionRole,
  switchConsoleSessionWorkspace as requestSwitchConsoleSessionWorkspace,
  type ApiBaseUrlLocation,
  type ConsoleSessionSnapshot,
  type ConsoleWorkspaceSummary
} from '@1flowbase/api-client/auth';

import { getAuthApiBaseUrl } from './session';

export function getScalarApiBaseUrl(
  locationLike: ApiBaseUrlLocation | undefined = typeof window !== 'undefined'
    ? window.location
    : undefined
): string {
  if (import.meta.env.VITE_SCALAR_API_BASE_URL) {
    return import.meta.env.VITE_SCALAR_API_BASE_URL;
  }

  if (locationLike?.origin) {
    return locationLike.origin;
  }

  const protocol = locationLike?.protocol === 'https:' ? 'https:' : 'http:';
  const hostname = locationLike?.hostname || '127.0.0.1';
  const port = locationLike?.port;

  return port ? `${protocol}//${hostname}:${port}` : `${protocol}//${hostname}`;
}

export function signOut(
  csrfToken: string,
  baseUrl = getAuthApiBaseUrl()
): Promise<void> {
  return requestDeleteConsoleSession(csrfToken, baseUrl);
}

export function switchActiveRole(
  roleCode: string,
  csrfToken: string,
  baseUrl = getAuthApiBaseUrl()
): Promise<ConsoleSessionSnapshot> {
  return requestSwitchConsoleSessionRole(roleCode, csrfToken, baseUrl);
}

export function fetchAccessibleWorkspaces(
  baseUrl = getAuthApiBaseUrl()
): Promise<ConsoleWorkspaceSummary[]> {
  return requestFetchConsoleWorkspaces(baseUrl);
}

export function switchWorkspace(
  workspaceId: string,
  csrfToken: string,
  baseUrl = getAuthApiBaseUrl()
): Promise<ConsoleSessionSnapshot> {
  return requestSwitchConsoleSessionWorkspace(workspaceId, csrfToken, baseUrl);
}

export function redeemWorkspaceInvitation(
  invitationToken: string,
  csrfToken: string,
  baseUrl = getAuthApiBaseUrl()
): Promise<ConsoleSessionSnapshot> {
  return requestRedeemConsoleWorkspaceInvitation(
    invitationToken,
    csrfToken,
    baseUrl
  );
}
