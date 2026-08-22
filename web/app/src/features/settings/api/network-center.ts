import {
  createConsoleNetworkEgressProvider,
  createConsoleNetworkEgressProxy,
  createConsoleNetworkEgressPool,
  createConsoleNetworkEgressPoolMember,
  createConsoleNetworkEgressPoolStaticHttpMember,
  addConsoleNetworkEgressProviderToPool,
  deleteConsoleNetworkEgressPool,
  deleteConsoleNetworkEgressPoolMember,
  createConsoleNetworkEgressRoute,
  deleteConsoleNetworkEgressRoute,
  listConsoleNetworkEgressProviders,
  listConsoleNetworkEgressProviderTypes,
  listConsoleNetworkEgressOfficialPluginCatalog,
  listConsoleNetworkEgressPools,
  listConsoleNetworkEgressRoutes,
  updateConsoleNetworkEgressPool,
  updateConsoleNetworkEgressPoolMember,
  updateConsoleNetworkEgressRoute,
  updateConsoleNetworkEgressProviderLifecycle,
  syncConsoleNetworkEgressProvider,
  testConsoleNetworkEgressPoolMember,
  installConsoleNetworkEgressOfficialPlugin,
  uploadConsoleNetworkEgressPluginPackage,
  type ConsoleNetworkEgressPool,
  type ConsoleNetworkEgressPoolMember,
  type ConsoleNetworkEgressProvider,
  type ConsoleNetworkEgressProviderType,
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
  type UpdateConsoleNetworkEgressRouteInput,
  type UpdateConsoleNetworkEgressProviderLifecycleInput
} from '@1flowbase/api-client';

export type SettingsNetworkEgressProvider = ConsoleNetworkEgressProvider;
export type SettingsNetworkEgressProviderType =
  ConsoleNetworkEgressProviderType;
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
  return installConsoleNetworkEgressOfficialPlugin({ plugin_id: pluginId }, csrfToken);
}

export function uploadSettingsNetworkEgressPluginPackage(
  file: File,
  csrfToken: string
) {
  return uploadConsoleNetworkEgressPluginPackage(file, csrfToken);
}

export function createSettingsNetworkEgressProvider(
  input: CreateSettingsNetworkEgressProviderInput,
  csrfToken: string
) {
  return createConsoleNetworkEgressProvider(input, csrfToken);
}

export function updateSettingsNetworkEgressProviderLifecycle(
  providerId: string,
  input: UpdateSettingsNetworkEgressProviderLifecycleInput,
  csrfToken: string
) {
  return updateConsoleNetworkEgressProviderLifecycle(
    providerId,
    input,
    csrfToken
  );
}

export function syncSettingsNetworkEgressProvider(
  providerId: string,
  csrfToken: string
) {
  return syncConsoleNetworkEgressProvider(providerId, csrfToken);
}

export function fetchSettingsNetworkEgressPools() {
  return listConsoleNetworkEgressPools();
}
export function createSettingsNetworkEgressProxy(
  input: CreateSettingsNetworkEgressProxyInput,
  csrfToken: string
) {
  return createConsoleNetworkEgressProxy(input, csrfToken);
}
export function testSettingsNetworkEgressPoolMember(
  poolId: string,
  memberId: string,
  csrfToken: string
) {
  return testConsoleNetworkEgressPoolMember(poolId, memberId, csrfToken);
}
export function fetchSettingsNetworkEgressRoutes() {
  return listConsoleNetworkEgressRoutes();
}
export function createSettingsNetworkEgressRoute(
  input: CreateSettingsNetworkEgressRouteInput,
  csrfToken: string
) {
  return createConsoleNetworkEgressRoute(input, csrfToken);
}
export function updateSettingsNetworkEgressRoute(
  routeId: string,
  input: UpdateSettingsNetworkEgressRouteInput,
  csrfToken: string
) {
  return updateConsoleNetworkEgressRoute(routeId, input, csrfToken);
}
export function deleteSettingsNetworkEgressRoute(
  routeId: string,
  csrfToken: string
) {
  return deleteConsoleNetworkEgressRoute(routeId, csrfToken);
}

export function createSettingsNetworkEgressPool(
  input: CreateSettingsNetworkEgressPoolInput,
  csrfToken: string
) {
  return createConsoleNetworkEgressPool(input, csrfToken);
}

export function updateSettingsNetworkEgressPool(
  poolId: string,
  input: UpdateSettingsNetworkEgressPoolInput,
  csrfToken: string
) {
  return updateConsoleNetworkEgressPool(poolId, input, csrfToken);
}

export function deleteSettingsNetworkEgressPool(
  poolId: string,
  csrfToken: string
) {
  return deleteConsoleNetworkEgressPool(poolId, csrfToken);
}

export function createSettingsNetworkEgressPoolMember(
  poolId: string,
  input: CreateSettingsNetworkEgressPoolMemberInput,
  csrfToken: string
) {
  return createConsoleNetworkEgressPoolMember(poolId, input, csrfToken);
}

export function createSettingsNetworkEgressPoolStaticHttpMember(
  poolId: string,
  input: CreateSettingsNetworkEgressPoolStaticHttpMemberInput,
  csrfToken: string
) {
  return createConsoleNetworkEgressPoolStaticHttpMember(poolId, input, csrfToken);
}

export function addSettingsNetworkEgressProviderToPool(
  poolId: string,
  input: AddSettingsNetworkEgressProviderToPoolInput,
  csrfToken: string
) {
  return addConsoleNetworkEgressProviderToPool(poolId, input, csrfToken);
}

export function updateSettingsNetworkEgressPoolMember(
  poolId: string,
  memberId: string,
  input: UpdateSettingsNetworkEgressPoolMemberInput,
  csrfToken: string
) {
  return updateConsoleNetworkEgressPoolMember(
    poolId,
    memberId,
    input,
    csrfToken
  );
}

export function deleteSettingsNetworkEgressPoolMember(
  poolId: string,
  memberId: string,
  csrfToken: string
) {
  return deleteConsoleNetworkEgressPoolMember(poolId, memberId, csrfToken);
}
