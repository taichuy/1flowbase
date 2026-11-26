import {
  fetchConsoleMe as requestFetchConsoleMe,
  fetchConsoleSession as requestFetchConsoleSession,
  fetchPublicLoginEntries as requestFetchPublicLoginEntries,
  getDefaultApiBaseUrl,
  signInWithPassword as requestSignInWithPassword,
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
