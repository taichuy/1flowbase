import { getAuthApiBaseUrl } from '../api/session';

export function redirectToExternalLogin(
  loginEntryId: string,
  redirect: (url: string) => void = (url) => window.location.assign(url),
  invitationToken?: string
): void {
  const url = new URL('/api/public/auth/external/start', getAuthApiBaseUrl());
  url.searchParams.set('login_entry_id', loginEntryId);
  if (invitationToken?.trim()) {
    url.searchParams.set('invitation_token', invitationToken.trim());
  }
  redirect(url.toString());
}
