import {
  fetchConsoleRoleDataPolicy,
  listConsoleRoles,
  replaceConsoleRoleDataPolicy,
  type ConsoleRole,
  type ConsoleRoleConsolePolicy,
  type ConsoleRoleDataPolicyScope,
  type ConsoleRolePermissions,
  type ConsoleRoleFrontstageRoutes,
  type ReplaceConsoleRoleConsolePolicyInput
} from '@1flowbase/api-client';

export {
  createConsoleRole as createSettingsRole,
  updateConsoleRole as updateSettingsRole,
  deleteConsoleRole as deleteSettingsRole,
  fetchConsoleRoleConsolePolicy as fetchSettingsRoleConsolePolicy,
  replaceConsoleRoleConsolePolicy as replaceSettingsRoleConsolePolicy,
  fetchConsoleRolePermissions as fetchSettingsRolePermissions,
  replaceConsoleRolePermissions as replaceSettingsRolePermissions,
  fetchConsoleRoleFrontstageRoutes as fetchSettingsRoleFrontstageRoutes,
  replaceConsoleRoleFrontstageRoutes as replaceSettingsRoleFrontstageRoutes
} from '@1flowbase/api-client';

export type SettingsRole = ConsoleRole;
export type SettingsRoleConsolePolicy = ConsoleRoleConsolePolicy;
export type ReplaceSettingsRoleConsolePolicyInput =
  ReplaceConsoleRoleConsolePolicyInput;
export type SettingsRolePermissions = ConsoleRolePermissions;
export type SettingsRoleFrontstageRoutes = ConsoleRoleFrontstageRoutes;
export type SettingsRoleDataPolicyScope = ConsoleRoleDataPolicyScope;
export type SettingsRoleDataPolicyWriteScope = Exclude<
  SettingsRoleDataPolicyScope,
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
  default_update_scope: SettingsRoleDataPolicyWriteScope;
  default_delete_scope: SettingsRoleDataPolicyWriteScope;
}

export interface SettingsRoleModelDataPolicy {
  data_model_id: string;
  can_create_override: boolean | null;
  view_scope_override: SettingsRoleDataPolicyOverrideScope;
  update_scope_override: SettingsRoleDataPolicyWriteScope | null;
  delete_scope_override: SettingsRoleDataPolicyWriteScope | null;
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
export const settingsRoleConsolePolicyQueryKey = (roleCode: string) =>
  ['settings', 'roles', roleCode, 'console-policy'] as const;
export const settingsRolePermissionsQueryKey = (roleCode: string) =>
  ['settings', 'roles', roleCode, 'permissions'] as const;
export const settingsRoleFrontstageRoutesQueryKey = (roleCode: string) =>
  ['settings', 'roles', roleCode, 'frontstage-routes'] as const;
export const settingsRoleDataPolicyQueryKey = (roleCode: string) =>
  ['settings', 'roles', roleCode, 'data-policy'] as const;

export function fetchSettingsRoles(): Promise<SettingsRole[]> {
  return listConsoleRoles();
}

export function fetchSettingsRoleDataPolicy(
  roleCode: string
): Promise<SettingsRoleDataPolicy> {
  return fetchConsoleRoleDataPolicy(
    roleCode
  );
}

export function replaceSettingsRoleDataPolicy(
  roleCode: string,
  input: ReplaceSettingsRoleDataPolicyInput,
  csrfToken: string
): Promise<SettingsRoleDataPolicy> {
  return replaceConsoleRoleDataPolicy(
    roleCode,
    input,
    csrfToken
  );
}
