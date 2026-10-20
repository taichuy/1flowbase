import { apiFetch, apiFetchVoid } from '../../transport';

export type ConsoleUserMeta = Record<string, unknown>;

export interface ConsoleMe {
  id: string;
  account: string;
  email: string;
  phone: string | null;
  nickname: string;
  name: string;
  avatar_url: string | null;
  introduction: string;
  preferred_locale?: string | null;
  meta?: ConsoleUserMeta;
  effective_display_role: string;
  permissions: string[];
}

export interface UpdateConsoleMeInput {
  name: string;
  nickname: string;
  email: string;
  phone: string | null;
  avatar_url: string | null;
  introduction: string;
  preferred_locale?: string | null;
}

export interface ChangeConsolePasswordInput {
  old_password: string;
  new_password: string;
}

export function authorizeConsoleExternalIdentity(
  input: { login_entry_id: string },
  csrfToken: string,
  baseUrl?: string
): Promise<{ authorization_url: string }> {
  return apiFetch({
    path: '/api/console/me/external-identities/authorize',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleMe(baseUrl?: string): Promise<ConsoleMe> {
  return apiFetch<ConsoleMe>({
    path: '/api/console/me',
    baseUrl
  });
}

export function updateConsoleMe(
  input: UpdateConsoleMeInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleMe> {
  return apiFetch<ConsoleMe>({
    path: '/api/console/me',
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function patchConsoleMeMeta(
  meta: ConsoleUserMeta,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleMe> {
  return apiFetch<ConsoleMe>({
    path: '/api/console/me/meta',
    method: 'PATCH',
    body: { meta },
    csrfToken,
    baseUrl
  });
}

export function changeConsolePassword(
  input: ChangeConsolePasswordInput,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: '/api/console/me/actions/change-password',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export interface AuthenticationVerificationEntry {
  id: string;
  title: string;
}

export interface ConsoleExternalIdentityBinding {
  identity_id: string;
  state: 'bound' | 'unlinked';
  subject_type: string;
  subject_value: string;
  issuer: string | null;
  account: string | null;
  nickname: string | null;
  avatar_url: string | null;
  profile_url: string | null;
  bound_at: string;
  unlinked_at: string | null;
  provider_available: boolean;
  can_unlink: boolean;
  unlink_blocked_reason: string | null;
}

export interface ConsoleAuthenticationProvider {
  auth_type: string;
  display_name: string;
  provider_available: boolean;
  login_entries: AuthenticationVerificationEntry[];
  bindings: ConsoleExternalIdentityBinding[];
}

export interface ConsoleAuthenticationMethods {
  password_configured: boolean;
  password_usable: boolean;
  password_setup_verified: boolean;
  password_verification_entries: AuthenticationVerificationEntry[];
  providers: ConsoleAuthenticationProvider[];
}

export function fetchConsoleAuthenticationMethods(
  baseUrl?: string
): Promise<ConsoleAuthenticationMethods> {
  return apiFetch({
    path: '/api/console/me/authentication-methods',
    baseUrl
  });
}

export function unlinkConsoleExternalIdentity(
  identity_id: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: `/api/console/me/authentication-methods/external-identities/${encodeURIComponent(identity_id)}/unlink`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function authorizeConsolePasswordSetup(
  input: { login_entry_id: string },
  csrfToken: string,
  baseUrl?: string
): Promise<{ authorization_url: string }> {
  return apiFetch({
    path: '/api/console/me/authentication-methods/password/authorize',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function setConsoleFirstPassword(
  input: { new_password: string },
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: '/api/console/me/authentication-methods/password',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}
