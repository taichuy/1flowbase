import {
  type ConsoleNetworkEgressProxy,
  listConsoleNetworkEgressProviders,
  listConsoleNetworkEgressProviderTypes,
  listConsoleNetworkEgressOfficialPluginCatalog,
  listConsoleNetworkEgressPools,
  listConsoleNetworkEgressRoutes,
  installConsoleNetworkEgressOfficialPlugin,
  listConsoleNetworkEgressPluginFamilies,
  type ConsoleNetworkEgressPool,
  type ConsoleNetworkEgressPoolMember,
  type ConsoleNetworkEgressProvider,
  type ConsoleNetworkEgressProviderType,
  type ConsoleNetworkEgressPluginFamily,
  type ConsoleNetworkEgressRoute,
  type CreateConsoleNetworkEgressPoolInput,
  type CreateConsoleNetworkEgressPoolMemberInput,
  type CreateConsoleNetworkEgressPoolStaticHttpMemberInput,
  type AddConsoleNetworkEgressProviderToPoolInput,
  type CreateConsoleNetworkEgressRouteInput,
  type CreateConsoleNetworkEgressProviderInput,
  type CreateConsoleNetworkEgressProxyInput,
  type UpdateConsoleNetworkEgressPoolInput,
  type UpdateConsoleNetworkEgressPoolMemberInput,
  type DeleteConsoleNetworkEgressPoolMembersInput,
  type UpdateConsoleNetworkEgressRouteInput,
  type UpdateConsoleNetworkEgressProviderLifecycleInput
} from '@1flowbase/api-client';

export {
  switchConsoleNetworkEgressPluginVersion as switchSettingsNetworkEgressPluginVersion,
  uninstallConsoleNetworkEgressPluginVersion as uninstallSettingsNetworkEgressPluginVersion,
  uninstallConsoleNetworkEgressPluginFamily as uninstallSettingsNetworkEgressPluginFamily,
  uploadConsoleNetworkEgressPluginPackage as uploadSettingsNetworkEgressPluginPackage,
  createConsoleNetworkEgressProvider as createSettingsNetworkEgressProvider,
  updateConsoleNetworkEgressProviderLifecycle as updateSettingsNetworkEgressProviderLifecycle,
  syncConsoleNetworkEgressProvider as syncSettingsNetworkEgressProvider,
  createConsoleNetworkEgressProxy as createSettingsNetworkEgressProxy,
  testConsoleNetworkEgressPoolMember as testSettingsNetworkEgressPoolMember,
  createConsoleNetworkEgressRoute as createSettingsNetworkEgressRoute,
  updateConsoleNetworkEgressRoute as updateSettingsNetworkEgressRoute,
  deleteConsoleNetworkEgressRoute as deleteSettingsNetworkEgressRoute,
  createConsoleNetworkEgressPool as createSettingsNetworkEgressPool,
  updateConsoleNetworkEgressPool as updateSettingsNetworkEgressPool,
  deleteConsoleNetworkEgressPool as deleteSettingsNetworkEgressPool,
  createConsoleNetworkEgressPoolMember as createSettingsNetworkEgressPoolMember,
  createConsoleNetworkEgressPoolStaticHttpMember as createSettingsNetworkEgressPoolStaticHttpMember,
  addConsoleNetworkEgressProviderToPool as addSettingsNetworkEgressProviderToPool,
  updateConsoleNetworkEgressPoolMember as updateSettingsNetworkEgressPoolMember,
  deleteConsoleNetworkEgressPoolMember as deleteSettingsNetworkEgressPoolMember,
  deleteConsoleNetworkEgressPoolMembers as deleteSettingsNetworkEgressPoolMembers,
  getConsoleNetworkEgressProxy as fetchSettingsNetworkEgressProxy,
  updateConsoleNetworkEgressProxy as updateSettingsNetworkEgressProxy
} from '@1flowbase/api-client';

export type SettingsNetworkEgressProvider = ConsoleNetworkEgressProvider;
export type SettingsNetworkEgressProviderType =
  ConsoleNetworkEgressProviderType;
export type SettingsNetworkEgressPluginFamily =
  ConsoleNetworkEgressPluginFamily;
export type CreateSettingsNetworkEgressProviderInput =
  CreateConsoleNetworkEgressProviderInput;
export type CreateSettingsNetworkEgressProxyInput =
  CreateConsoleNetworkEgressProxyInput;
export type UpdateSettingsNetworkEgressProviderLifecycleInput =
  UpdateConsoleNetworkEgressProviderLifecycleInput;
export type SettingsNetworkEgressPool = ConsoleNetworkEgressPool;
export type SettingsNetworkEgressPoolMember = ConsoleNetworkEgressPoolMember;
export type CreateSettingsNetworkEgressPoolInput =
  CreateConsoleNetworkEgressPoolInput;
export type UpdateSettingsNetworkEgressPoolInput =
  UpdateConsoleNetworkEgressPoolInput;
export type CreateSettingsNetworkEgressPoolMemberInput =
  CreateConsoleNetworkEgressPoolMemberInput;
export type CreateSettingsNetworkEgressPoolStaticHttpMemberInput =
  CreateConsoleNetworkEgressPoolStaticHttpMemberInput;
export type AddSettingsNetworkEgressProviderToPoolInput =
  AddConsoleNetworkEgressProviderToPoolInput;
export type UpdateSettingsNetworkEgressPoolMemberInput =
  UpdateConsoleNetworkEgressPoolMemberInput;
export type DeleteSettingsNetworkEgressPoolMembersInput =
  DeleteConsoleNetworkEgressPoolMembersInput;
export type SettingsNetworkEgressRoute = ConsoleNetworkEgressRoute;
export type CreateSettingsNetworkEgressRouteInput =
  CreateConsoleNetworkEgressRouteInput;
export type UpdateSettingsNetworkEgressRouteInput =
  UpdateConsoleNetworkEgressRouteInput;

export const settingsNetworkEgressProvidersQueryKey = [
  'settings',
  'network-center',
  'providers'
] as const;

export const settingsNetworkEgressPoolsQueryKey = [
  'settings',
  'network-center',
  'pools'
] as const;
export const settingsNetworkEgressRoutesQueryKey = [
  'settings',
  'network-center',
  'routes'
] as const;
export const settingsNetworkEgressProviderTypesQueryKey = [
  'settings',
  'network-center',
  'provider-types'
] as const;
export const settingsNetworkEgressOfficialPluginsQueryKey = [
  'settings',
  'network-center',
  'proxy-plugins',
  'official-catalog'
] as const;
export const settingsNetworkEgressPluginFamiliesQueryKey = [
  'settings',
  'network-center',
  'proxy-plugins',
  'families'
] as const;

export function fetchSettingsNetworkEgressProviders() {
  return listConsoleNetworkEgressProviders();
}

export function fetchSettingsNetworkEgressProviderTypes() {
  return listConsoleNetworkEgressProviderTypes();
}

export function fetchSettingsNetworkEgressOfficialPluginCatalog(
  options: { locale?: string; q?: string; cursor?: string; limit?: number } = {}
) {
  return listConsoleNetworkEgressOfficialPluginCatalog(options);
}

export function installSettingsNetworkEgressOfficialPlugin(
  pluginId: string,
  csrfToken: string
) {
  return installConsoleNetworkEgressOfficialPlugin(
    { plugin_id: pluginId },
    csrfToken
  );
}

export function fetchSettingsNetworkEgressPluginFamilies() {
  return listConsoleNetworkEgressPluginFamilies();
}

export function fetchSettingsNetworkEgressPools() {
  return listConsoleNetworkEgressPools();
}

export function fetchSettingsNetworkEgressRoutes() {
  return listConsoleNetworkEgressRoutes();
}

export type SettingsNetworkEgressProxy = ConsoleNetworkEgressProxy;
