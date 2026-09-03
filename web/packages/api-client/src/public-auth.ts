import { apiFetch } from './transport';

export interface PasswordSignInInput {
  identifier: string;
  password: string;
  login_entry_id?: string;
}

export interface PasswordSignInResponse {
  csrf_token: string;
  effective_display_role: string;
  current_workspace_id: string;
}

export interface PublicLoginEntry {
  id: string;
  auth_type: string;
  is_builtin: boolean;
  title: string;
  description?: string | null;
  sort_order: number;
  public_ui_block: string;
  public_variables: Record<string, unknown>;
}

export interface PublicLoginEntriesResponse {
  default_login_entry_id: string;
  login_entries: PublicLoginEntry[];
}

export function fetchPublicLoginEntries(
  baseUrl?: string
): Promise<PublicLoginEntriesResponse> {
  return apiFetch<PublicLoginEntriesResponse>({
    path: '/api/public/auth/login-entries',
    baseUrl
  });
}

export function signInWithPassword(
  input: PasswordSignInInput,
  baseUrl?: string
): Promise<PasswordSignInResponse> {
  return apiFetch<PasswordSignInResponse>({
    path: '/api/public/auth/sign-in',
    method: 'POST',
    body: input,
    baseUrl
  });
}
