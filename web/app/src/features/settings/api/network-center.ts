import {
  createConsoleNetworkEgressPool,
  createConsoleNetworkEgressPoolMember,
  deleteConsoleNetworkEgressPool,
  deleteConsoleNetworkEgressPoolMember,
  listConsoleNetworkEgressProviders,
  listConsoleNetworkEgressPools,
  updateConsoleNetworkEgressPool,
  updateConsoleNetworkEgressPoolMember,
  type ConsoleNetworkEgressPool,
  type ConsoleNetworkEgressPoolMember,
  type ConsoleNetworkEgressProvider,
  type CreateConsoleNetworkEgressPoolInput,
  type CreateConsoleNetworkEgressPoolMemberInput,
  type UpdateConsoleNetworkEgressPoolInput,
  type UpdateConsoleNetworkEgressPoolMemberInput
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

export function fetchSettingsNetworkEgressProviders() {
  return listConsoleNetworkEgressProviders();
}

export function fetchSettingsNetworkEgressPools() {
  return listConsoleNetworkEgressPools();
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
