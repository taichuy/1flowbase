import {
  fetchConsoleRoleConsolePolicyCatalog,
  listConsolePermissions,
  type ConsolePermission,
  type ConsolePolicyCatalog,
  type ConsolePolicyLocale
} from '@1flowbase/api-client';

export type SettingsPermission = ConsolePermission;
export type SettingsConsolePolicyCatalog = ConsolePolicyCatalog;

export const settingsPermissionsQueryKey = ['settings', 'permissions'] as const;
export const settingsConsolePolicyCatalogQueryKey = (
  locale: ConsolePolicyLocale
) => ['settings', 'console-policy-catalog', locale] as const;

export function fetchSettingsConsolePolicyCatalog(
  locale: ConsolePolicyLocale
): Promise<SettingsConsolePolicyCatalog> {
  return fetchConsoleRoleConsolePolicyCatalog(locale);
}

export function fetchSettingsPermissions(): Promise<SettingsPermission[]> {
  return listConsolePermissions();
}
