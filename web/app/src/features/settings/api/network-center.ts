import {
  createConsoleNetworkEgressPool,
  createConsoleNetworkEgressPoolMember,
  deleteConsoleNetworkEgressPool,
  deleteConsoleNetworkEgressPoolMember,
  createConsoleNetworkEgressRoute,
  deleteConsoleNetworkEgressRoute,
  listConsoleNetworkEgressProviders,
  listConsoleNetworkEgressPools,
  listConsoleNetworkEgressRoutes,
  updateConsoleNetworkEgressPool,
  updateConsoleNetworkEgressPoolMember,
  updateConsoleNetworkEgressRoute,
  type ConsoleNetworkEgressPool,
  type ConsoleNetworkEgressPoolMember,
  type ConsoleNetworkEgressProvider,
  type ConsoleNetworkEgressRoute,
  type CreateConsoleNetworkEgressPoolInput,
  type CreateConsoleNetworkEgressPoolMemberInput,
  type CreateConsoleNetworkEgressRouteInput,
  type UpdateConsoleNetworkEgressPoolInput,
  type UpdateConsoleNetworkEgressPoolMemberInput,
  type UpdateConsoleNetworkEgressRouteInput
} from '@1flowbase/api-client';

export type SettingsNetworkEgressProvider = ConsoleNetworkEgressProvider;
export type SettingsNetworkEgressPool = ConsoleNetworkEgressPool;
export type SettingsNetworkEgressPoolMember = ConsoleNetworkEgressPoolMember;
export type CreateSettingsNetworkEgressPoolInput =
  CreateConsoleNetworkEgressPoolInput;
export type UpdateSettingsNetworkEgressPoolInput =
  UpdateConsoleNetworkEgressPoolInput;
export type CreateSettingsNetworkEgressPoolMemberInput =
  CreateConsoleNetworkEgressPoolMemberInput;
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

export function fetchSettingsNetworkEgressProviders() {
  return listConsoleNetworkEgressProviders();
}

export function fetchSettingsNetworkEgressPools() {
  return listConsoleNetworkEgressPools();
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
