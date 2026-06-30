import { apiFetch } from './transport';

export interface PasswordSignInInput {
  identifier: string;
  password: string;
  authenticator_id?: string;
}

export interface PasswordSignInResponse {
  csrf_token: string;
  effective_display_role: string;
  current_workspace_id: string;
}

export interface PublicLoginInstance {
  id: string;
  auth_type: string;
  title: string;
  description?: string | null;
  sort_order: number;
  flow: string;
  sign_in_path: string;
  public_options: Record<string, unknown>;
}

export interface PublicLoginInstancesResponse {
  default_authenticator_id: string;
  login_instances: PublicLoginInstance[];
}

export function fetchPublicLoginInstances(
  baseUrl?: string
): Promise<PublicLoginInstancesResponse> {
  return apiFetch<PublicLoginInstancesResponse>({
    path: '/api/public/auth/login-instances',
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
