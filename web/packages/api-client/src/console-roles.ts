import { apiFetch, apiFetchVoid } from './transport';

export interface ConsoleRole {
  code: string;
  name: string;
  introduction: string;
  scope_kind: 'system' | 'workspace';
  is_builtin: boolean;
  is_editable: boolean;
  auto_grant_new_permissions: boolean;
  is_default_member_role: boolean;
  permission_codes: string[];
}

export interface ConsoleRolePermissions {
  role_code: string;
  permission_codes: string[];
}

export type ConsoleRoleDataPolicyScope = 'own' | 'scope_all' | 'system_all';
export type ConsoleRoleDataPolicyOverrideScope =
  | ConsoleRoleDataPolicyScope
  | null;

export interface ConsoleRoleDefaultDataPolicy {
  can_view: boolean;
  can_create: boolean;
  can_update: boolean;
  can_delete: boolean;
  default_view_scope: ConsoleRoleDataPolicyScope;
  default_update_scope: ConsoleRoleDataPolicyScope;
  default_delete_scope: ConsoleRoleDataPolicyScope;
}

export interface ConsoleRoleModelDataPolicy {
  data_model_id: string;
  can_create_override: boolean | null;
  view_scope_override: ConsoleRoleDataPolicyOverrideScope;
  update_scope_override: ConsoleRoleDataPolicyOverrideScope;
  delete_scope_override: ConsoleRoleDataPolicyOverrideScope;
}

export interface ConsoleRoleDataPolicy {
  role_code: string;
  default_policy: ConsoleRoleDefaultDataPolicy;
  model_policies: ConsoleRoleModelDataPolicy[];
}

export interface ReplaceConsoleRoleDataPolicyInput {
  default_policy: ConsoleRoleDefaultDataPolicy;
  model_policies: ConsoleRoleModelDataPolicy[];
}

export interface CreateConsoleRoleInput {
  code: string;
  name: string;
  introduction: string;
  auto_grant_new_permissions?: boolean;
  is_default_member_role?: boolean;
}

export interface UpdateConsoleRoleInput {
  name: string;
  introduction: string;
  auto_grant_new_permissions?: boolean;
  is_default_member_role?: boolean;
}

export interface ReplaceConsoleRolePermissionsInput {
  permission_codes: string[];
}

export function listConsoleRoles(baseUrl?: string): Promise<ConsoleRole[]> {
  return apiFetch<ConsoleRole[]>({
    path: '/api/console/roles',
    baseUrl
  });
}

export function createConsoleRole(
  input: CreateConsoleRoleInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleRole> {
  return apiFetch<ConsoleRole>({
    path: '/api/console/roles',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleRole(
  roleCode: string,
  input: UpdateConsoleRoleInput,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: `/api/console/roles/${roleCode}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleRole(
  roleCode: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: `/api/console/roles/${roleCode}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleRolePermissions(
  roleCode: string,
  baseUrl?: string
): Promise<ConsoleRolePermissions> {
  return apiFetch<ConsoleRolePermissions>({
    path: `/api/console/roles/${roleCode}/permissions`,
    baseUrl
  });
}

export function replaceConsoleRolePermissions(
  roleCode: string,
  input: ReplaceConsoleRolePermissionsInput,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: `/api/console/roles/${roleCode}/permissions`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleRoleDataPolicy(
  roleCode: string,
  baseUrl?: string
): Promise<ConsoleRoleDataPolicy> {
  return apiFetch<ConsoleRoleDataPolicy>({
    path: `/api/console/roles/${roleCode}/data-policy`,
    baseUrl
  });
}

export function replaceConsoleRoleDataPolicy(
  roleCode: string,
  input: ReplaceConsoleRoleDataPolicyInput,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: `/api/console/roles/${roleCode}/data-policy`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}
