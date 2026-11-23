import {
  deleteConsoleSession as requestDeleteConsoleSession,
  fetchConsoleMe as requestFetchConsoleMe,
  fetchConsoleSession as requestFetchConsoleSession,
  fetchConsoleWorkspaces as requestFetchConsoleWorkspaces,
  fetchPublicLoginEntries as requestFetchPublicLoginEntries,
  getDefaultApiBaseUrl,
  redeemConsoleWorkspaceInvitation as requestRedeemConsoleWorkspaceInvitation,
  signInWithPassword as requestSignInWithPassword,
  switchConsoleSessionRole as requestSwitchConsoleSessionRole,
  switchConsoleSessionWorkspace as requestSwitchConsoleSessionWorkspace,
  type ApiBaseUrlLocation,
  type ConsoleMe,
  type ConsoleSessionSnapshot,
  type ConsoleWorkspaceSummary,
  type PasswordSignInInput,
  type PasswordSignInResponse,
  type PublicLoginEntry,
  type PublicLoginEntriesResponse,
  type PublicOnboardingMode
} from '@1flowbase/api-client/auth';

export type {
  ConsoleWorkspaceSummary,
  PasswordSignInResponse,
  PublicLoginEntry,
  PublicOnboardingMode
};

const loginEntryFlights = new Map<
  string,
  Promise<PublicLoginEntriesResponse>
>();

export function getAuthApiBaseUrl(
  locationLike: ApiBaseUrlLocation | undefined = typeof window !== 'undefined'
    ? window.location
    : undefined
): string {
  return (
    import.meta.env.VITE_API_BASE_URL ?? getDefaultApiBaseUrl(locationLike)
  );
}

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

export function signInWithPassword(
  input: PasswordSignInInput,
  baseUrl = getAuthApiBaseUrl()
): Promise<PasswordSignInResponse> {
  return requestSignInWithPassword(input, baseUrl);
}

export function fetchLoginEntries(
  baseUrl = getAuthApiBaseUrl()
): Promise<PublicLoginEntriesResponse> {
  const existingFlight = loginEntryFlights.get(baseUrl);
  if (existingFlight) return existingFlight;

  const flight = requestFetchPublicLoginEntries(baseUrl).finally(() => {
    if (loginEntryFlights.get(baseUrl) === flight) {
      loginEntryFlights.delete(baseUrl);
    }
  });
  loginEntryFlights.set(baseUrl, flight);
  return flight;
}

export function fetchCurrentSession(
  baseUrl = getAuthApiBaseUrl()
): Promise<ConsoleSessionSnapshot> {
  return requestFetchConsoleSession(baseUrl);
}

export function fetchCurrentMe(
  baseUrl = getAuthApiBaseUrl()
): Promise<ConsoleMe> {
  return requestFetchConsoleMe(baseUrl);
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
