import { apiFetch, apiFetchVoid } from './transport';

export interface ConsoleMember {
  id: string;
  account: string;
  email: string;
  phone: string | null;
  name: string;
  nickname: string;
  introduction: string;
  default_display_role: string | null;
  email_login_enabled: boolean;
  phone_login_enabled: boolean;
  status: 'active' | 'disabled';
  role_codes: string[];
}

export interface CreateConsoleMemberInput {
  account: string;
  email: string;
  phone: string | null;
  password: string;
  name: string;
  nickname: string;
  introduction: string;
  email_login_enabled: boolean;
  phone_login_enabled: boolean;
}

export interface UpdateConsoleMemberInput {
  email: string;
  phone: string | null;
  name: string;
  nickname: string;
  introduction: string;
}

export interface ResetConsoleMemberPasswordInput {
  new_password: string;
}

export interface ReplaceConsoleMemberRolesInput {
  role_codes: string[];
}

export function listConsoleMembers(baseUrl?: string): Promise<ConsoleMember[]> {
  return apiFetch<ConsoleMember[]>({
    path: '/api/console/settings/members',
    baseUrl
  });
}

export function createConsoleMember(
  input: CreateConsoleMemberInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleMember> {
  return apiFetch<ConsoleMember>({
    path: '/api/console/settings/members',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleMember(
  memberId: string,
  input: UpdateConsoleMemberInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleMember> {
  return apiFetch<ConsoleMember>({
    path: `/api/console/settings/members/${memberId}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function disableConsoleMember(
  memberId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: `/api/console/settings/members/${memberId}/disable`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function enableConsoleMember(
  memberId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: `/api/console/settings/members/${memberId}/enable`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMember(
  memberId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: `/api/console/settings/members/${memberId}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function resetConsoleMemberPassword(
  memberId: string,
  input: ResetConsoleMemberPasswordInput,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: `/api/console/settings/members/${memberId}/reset-password`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function replaceConsoleMemberRoles(
  memberId: string,
  input: ReplaceConsoleMemberRolesInput,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: `/api/console/settings/members/${memberId}/roles`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}
