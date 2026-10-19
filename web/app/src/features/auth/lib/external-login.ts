import { getAuthApiBaseUrl } from '../api/session';

export function redirectToExternalLogin(
  loginEntryId: string,
  redirect: (url: string) => void = (url) => window.location.assign(url)
): void {
  const url = new URL('/api/public/auth/external/start', getAuthApiBaseUrl());
  url.searchParams.set('login_entry_id', loginEntryId);
  redirect(url.toString());
}
