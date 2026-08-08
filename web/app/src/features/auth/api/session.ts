import {
  deleteConsoleSession as requestDeleteConsoleSession,
  fetchConsoleMe as requestFetchConsoleMe,
  fetchConsoleSession as requestFetchConsoleSession,
  fetchPublicLoginInstances as requestFetchPublicLoginInstances,
  getDefaultApiBaseUrl,
  signInWithPassword as requestSignInWithPassword,
  switchConsoleSessionRole as requestSwitchConsoleSessionRole,
  type ApiBaseUrlLocation,
  type ConsoleMe,
  type ConsoleSessionSnapshot,
  type PasswordSignInInput,
  type PasswordSignInResponse,
  type PublicLoginInstance,
  type PublicLoginInstancesResponse
} from '@1flowbase/api-client';

export type { PasswordSignInResponse, PublicLoginInstance };

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

export function fetchLoginInstances(
  baseUrl = getAuthApiBaseUrl()
): Promise<PublicLoginInstancesResponse> {
  return requestFetchPublicLoginInstances(baseUrl);
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
