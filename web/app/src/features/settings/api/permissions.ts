import {
  fetchConsoleRoleConsolePolicyCatalog,
  listConsolePermissions,
  type ConsolePermission,
  type ConsolePolicyCatalog
} from '@1flowbase/api-client';

export type SettingsPermission = ConsolePermission;
export type SettingsConsolePolicyCatalog = ConsolePolicyCatalog;

export const settingsPermissionsQueryKey = ['settings', 'permissions'] as const;
export const settingsConsolePolicyCatalogQueryKey = [
  'settings',
  'console-policy-catalog'
] as const;

export function fetchSettingsConsolePolicyCatalog(): Promise<SettingsConsolePolicyCatalog> {
  return fetchConsoleRoleConsolePolicyCatalog();
}

export function fetchSettingsPermissions(): Promise<SettingsPermission[]> {
  return listConsolePermissions();
}
