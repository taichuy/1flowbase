import {
  fetchConsoleSystemRuntimeProcesses,
  fetchConsoleSystemRuntimeProfile,
  type ConsoleSystemRuntimeProcessList,
  type ConsoleSystemRuntimeProfile
} from '@1flowbase/api-client';

export { terminateConsoleSystemProcess as terminateSettingsSystemRuntimeProcess } from '@1flowbase/api-client';

export type SettingsSystemRuntimeProfile = ConsoleSystemRuntimeProfile;
export type SettingsSystemRuntimeProcessList = ConsoleSystemRuntimeProcessList;

export const settingsSystemRuntimeQueryKey = [
  'settings',
  'system-runtime'
] as const;

export const settingsSystemRuntimeProcessesQueryKey = [
  'settings',
  'system-runtime',
  'processes'
] as const;

export function fetchSettingsSystemRuntimeProfile() {
  return fetchConsoleSystemRuntimeProfile();
}

export function fetchSettingsSystemRuntimeProcesses() {
  return fetchConsoleSystemRuntimeProcesses();
}
