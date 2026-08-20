import {
  listConsoleNetworkEgressProviders,
  type ConsoleNetworkEgressProvider
} from '@1flowbase/api-client';

export type SettingsNetworkEgressProvider = ConsoleNetworkEgressProvider;

export const settingsNetworkEgressProvidersQueryKey = [
  'settings',
  'network-center',
  'providers'
] as const;

export function fetchSettingsNetworkEgressProviders() {
  return listConsoleNetworkEgressProviders();
}
