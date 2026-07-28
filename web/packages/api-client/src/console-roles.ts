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

export type ConsolePolicyGroupKind = 'settings_feature' | 'other';
export type ConsolePolicyStrategy = 'full' | 'custom';
export type ConsolePolicyRowScope = 'disabled' | 'own' | 'scope_all';
export type ConsolePolicyCatalogLocale = 'zh_Hans' | 'en_US';

export interface ConsolePolicyCatalogOption<TValue extends string = string> {
  value: TValue;
  label: string;
  description: string;
}

export interface ConsolePolicyOperationSimpleAuthorization {
  kind: 'simple';
}

export interface ConsolePolicyOperationResourceActionAuthorization {
  kind: 'resource_action';
  resource_code: string;
  action_code: string;
}

export type ConsolePolicyOperationAuthorization =
  | ConsolePolicyOperationSimpleAuthorization
  | ConsolePolicyOperationResourceActionAuthorization;

export interface ConsolePolicyCatalogSimpleFullProfile {
  kind: 'simple';
  enabled: boolean;
}

export interface ConsolePolicyCatalogRowFullProfile {
  kind: 'row';
  scope: ConsolePolicyRowScope;
}

export type ConsolePolicyCatalogFullProfile =
  | ConsolePolicyCatalogSimpleFullProfile
  | ConsolePolicyCatalogRowFullProfile;

export interface ConsolePolicyCatalogRoute {
  method: string;
  path: string;
}

export interface ConsolePolicyCatalogOperation {
  operation_id: string;
  label: string;
  description: string;
  order: number;
  route: ConsolePolicyCatalogRoute;
  full_profile: ConsolePolicyCatalogFullProfile;
  allowed_row_scopes: ConsolePolicyCatalogOption<ConsolePolicyRowScope>[];
  authorization: ConsolePolicyOperationAuthorization;
}

export interface ConsolePolicyCatalogGroup {
  kind: ConsolePolicyGroupKind;
  group_id: string;
  label: string;
  description: string;
  operations: ConsolePolicyCatalogOperation[];
}

export interface ConsolePolicyCatalogResourceAction {
  action_code: string;
  label: string;
  description: string;
}

export interface ConsolePolicyCatalogResource {
  resource_code: string;
  label: string;
  description: string;
  actions: ConsolePolicyCatalogResourceAction[];
}

export interface ConsolePolicyCatalog {
  schema_version: string;
  locale: ConsolePolicyCatalogLocale;
  group_strategy_options: ConsolePolicyCatalogOption<ConsolePolicyStrategy>[];
  groups: ConsolePolicyCatalogGroup[];
  resources: ConsolePolicyCatalogResource[];
}

export interface ConsoleRoleConsolePolicySimpleOperation {
  operation_id: string;
  kind: 'simple';
  enabled: boolean;
}

export interface ConsoleRoleConsolePolicyRowOperation {
  operation_id: string;
  kind: 'row';
  scope: ConsolePolicyRowScope;
}

export type ConsoleRoleConsolePolicyOperation =
  | ConsoleRoleConsolePolicySimpleOperation
  | ConsoleRoleConsolePolicyRowOperation;

export interface ConsoleRoleConsolePolicyGroup {
  kind: ConsolePolicyGroupKind;
  group_id: string;
  enabled: boolean;
  strategy: ConsolePolicyStrategy;
  operations: ConsoleRoleConsolePolicyOperation[];
}

export interface ConsoleRoleConsolePolicy {
  role_code: string;
  groups: ConsoleRoleConsolePolicyGroup[];
}

export type ReplaceConsoleRoleConsolePolicyInput = Pick<
  ConsoleRoleConsolePolicy,
  'groups'
>;

export interface ConsoleRoleFrontstageRouteNode {
  id: string;
  kind: 'group' | 'page' | 'tab';
  title: string | null;
  slug: string | null;
  children: ConsoleRoleFrontstageRouteNode[];
}

export interface ConsoleRoleFrontstageRoutes {
  role_code: string;
  checked_page_ids: string[];
  checked_tab_ids: string[];
  tree: ConsoleRoleFrontstageRouteNode[];
}

export interface ReplaceConsoleRoleFrontstageRoutesInput {
  page_ids: string[];
  tab_ids: string[];
}

export type ConsoleRoleDataPolicyScope = 'own' | 'scope_all' | 'system_all';
export type ConsoleRoleDataPolicyOverrideScope =
  ConsoleRoleDataPolicyScope | null;

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
    path: '/api/console/settings/roles',
    baseUrl
  });
}

export function createConsoleRole(
  input: CreateConsoleRoleInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleRole> {
  return apiFetch<ConsoleRole>({
    path: '/api/console/settings/roles',
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
    path: `/api/console/settings/roles/${roleCode}`,
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
    path: `/api/console/settings/roles/${roleCode}`,
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
    path: `/api/console/settings/roles/${roleCode}/permissions`,
    baseUrl
  });
}

function buildConsoleRoleConsolePolicyCatalogPath(
  locale: ConsolePolicyCatalogLocale
) {
  const params = new URLSearchParams();
  params.set('locale', locale);
  return `/api/console/settings/roles/console-policy-catalog?${params.toString()}`;
}

export function fetchConsoleRoleConsolePolicyCatalog(
  locale: ConsolePolicyCatalogLocale,
  baseUrl?: string
): Promise<ConsolePolicyCatalog> {
  return apiFetch<ConsolePolicyCatalog>({
    path: buildConsoleRoleConsolePolicyCatalogPath(locale),
    baseUrl
  });
}

export function fetchConsoleRoleConsolePolicy(
  roleCode: string,
  baseUrl?: string
): Promise<ConsoleRoleConsolePolicy> {
  return apiFetch<ConsoleRoleConsolePolicy>({
    path: `/api/console/settings/roles/${roleCode}/console-policy`,
    baseUrl
  });
}

export function replaceConsoleRoleConsolePolicy(
  roleCode: string,
  input: ReplaceConsoleRoleConsolePolicyInput,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: `/api/console/settings/roles/${roleCode}/console-policy`,
    method: 'PUT',
    body: input,
    csrfToken,
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
    path: `/api/console/settings/roles/${roleCode}/permissions`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleRoleFrontstageRoutes(
  roleCode: string,
  baseUrl?: string
): Promise<ConsoleRoleFrontstageRoutes> {
  return apiFetch<ConsoleRoleFrontstageRoutes>({
    path: `/api/console/settings/roles/${roleCode}/frontstage-routes`,
    baseUrl
  });
}

export function replaceConsoleRoleFrontstageRoutes(
  roleCode: string,
  input: ReplaceConsoleRoleFrontstageRoutesInput,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: `/api/console/settings/roles/${roleCode}/frontstage-routes`,
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
    path: `/api/console/settings/roles/${roleCode}/data-policy`,
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
    path: `/api/console/settings/roles/${roleCode}/data-policy`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}
