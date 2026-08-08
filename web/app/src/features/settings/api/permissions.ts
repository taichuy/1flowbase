import {
  fetchConsoleRoleConsolePolicyCatalog,
  listConsolePermissions,
  replaceConsoleSettingsOrder,
  type ConsolePermission,
  type ConsolePolicyCatalog,
  type ConsolePolicyCatalogLocale
} from '@1flowbase/api-client';

export type SettingsPermission = ConsolePermission;
export type SettingsConsolePolicyCatalog = ConsolePolicyCatalog;
export type SettingsConsolePolicyCatalogLocale = ConsolePolicyCatalogLocale;

export const settingsPermissionsQueryKey = ['settings', 'permissions'] as const;
export function settingsConsolePolicyCatalogQueryKey(
  locale: SettingsConsolePolicyCatalogLocale
) {
  return ['settings', 'console-policy-catalog', locale] as const;
}

export function replaceSettingsConsolePolicyOrder(
  expectedRevision: number,
  groupIds: string[],
  csrfToken: string,
  locale: SettingsConsolePolicyCatalogLocale
): Promise<SettingsConsolePolicyCatalog> {
  return replaceConsoleSettingsOrder(
    { expected_revision: expectedRevision, group_ids: groupIds },
    csrfToken,
    locale
  );
}

export function fetchSettingsConsolePolicyCatalog(
  locale: SettingsConsolePolicyCatalogLocale
): Promise<SettingsConsolePolicyCatalog> {
  return fetchConsoleRoleConsolePolicyCatalog(locale);
}

export function fetchSettingsPermissions(): Promise<SettingsPermission[]> {
  return listConsolePermissions();
}
