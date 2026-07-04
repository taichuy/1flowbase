import {
  createConsoleRole,
  deleteConsoleRole,
  fetchConsoleRoleDataPolicy,
  fetchConsoleRolePermissions,
  listConsoleRoles,
  replaceConsoleRoleDataPolicy,
  replaceConsoleRolePermissions,
  updateConsoleRole,
  type ConsoleRole,
  type ConsoleRoleDataPolicyScope,
  type ConsoleRolePermissions,
  type CreateConsoleRoleInput,
  type ReplaceConsoleRolePermissionsInput,
  type UpdateConsoleRoleInput
} from '@1flowbase/api-client';

export type SettingsRole = ConsoleRole;
export type SettingsRolePermissions = ConsoleRolePermissions;
export type SettingsRoleDataPolicyScope = Exclude<
  ConsoleRoleDataPolicyScope,
  'system_all'
>;
export type SettingsRoleDataPolicyOverrideScope =
  SettingsRoleDataPolicyScope | null;

export interface SettingsRoleDefaultDataPolicy {
  can_view: boolean;
  can_create: boolean;
  can_update: boolean;
  can_delete: boolean;
  default_view_scope: SettingsRoleDataPolicyScope;
  default_update_scope: SettingsRoleDataPolicyScope;
  default_delete_scope: SettingsRoleDataPolicyScope;
}

export interface SettingsRoleModelDataPolicy {
  data_model_id: string;
  view_scope_override: SettingsRoleDataPolicyOverrideScope;
  update_scope_override: SettingsRoleDataPolicyOverrideScope;
  delete_scope_override: SettingsRoleDataPolicyOverrideScope;
}

export interface SettingsRoleDataPolicy {
  role_code: string;
  default_policy: SettingsRoleDefaultDataPolicy;
  model_policies: SettingsRoleModelDataPolicy[];
}

export interface ReplaceSettingsRoleDataPolicyInput {
  default_policy: SettingsRoleDefaultDataPolicy;
  model_policies: SettingsRoleModelDataPolicy[];
}

export const settingsRolesQueryKey = ['settings', 'roles'] as const;
export const settingsRolePermissionsQueryKey = (roleCode: string) =>
  ['settings', 'roles', roleCode, 'permissions'] as const;
export const settingsRoleDataPolicyQueryKey = (roleCode: string) =>
  ['settings', 'roles', roleCode, 'data-policy'] as const;

export function fetchSettingsRoles(): Promise<SettingsRole[]> {
  return listConsoleRoles();
}

export function createSettingsRole(
  input: CreateConsoleRoleInput,
  csrfToken: string
): Promise<SettingsRole> {
  return createConsoleRole(input, csrfToken);
}

export function updateSettingsRole(
  roleCode: string,
  input: UpdateConsoleRoleInput,
  csrfToken: string
): Promise<void> {
  return updateConsoleRole(roleCode, input, csrfToken);
}

export function deleteSettingsRole(roleCode: string, csrfToken: string): Promise<void> {
  return deleteConsoleRole(roleCode, csrfToken);
}

export function fetchSettingsRolePermissions(
  roleCode: string
): Promise<SettingsRolePermissions> {
  return fetchConsoleRolePermissions(roleCode);
}

export function replaceSettingsRolePermissions(
  roleCode: string,
  input: ReplaceConsoleRolePermissionsInput,
  csrfToken: string
): Promise<void> {
  return replaceConsoleRolePermissions(roleCode, input, csrfToken);
}

export function fetchSettingsRoleDataPolicy(
  roleCode: string
): Promise<SettingsRoleDataPolicy> {
  return fetchConsoleRoleDataPolicy(roleCode) as Promise<SettingsRoleDataPolicy>;
}

export function replaceSettingsRoleDataPolicy(
  roleCode: string,
  input: ReplaceSettingsRoleDataPolicyInput,
  csrfToken: string
): Promise<void> {
  return replaceConsoleRoleDataPolicy(roleCode, input, csrfToken);
}
