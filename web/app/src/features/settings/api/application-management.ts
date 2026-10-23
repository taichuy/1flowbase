import {
  type ConsoleApplicationManagementItem,
  type ConsoleApplicationManagementPage,
  type ConsoleApplicationManagementQuery
} from '@1flowbase/api-client';

export { listConsoleApplicationManagement as fetchSettingsApplicationManagement } from '@1flowbase/api-client';

export type SettingsApplicationManagementItem =
  ConsoleApplicationManagementItem;
export type SettingsApplicationManagementPage =
  ConsoleApplicationManagementPage;
export type SettingsApplicationManagementQuery =
  ConsoleApplicationManagementQuery;

export const settingsApplicationManagementQueryPrefix = [
  'settings',
  'applications'
] as const;

export function settingsApplicationManagementQueryKey(
  query: SettingsApplicationManagementQuery
) {
  return [...settingsApplicationManagementQueryPrefix, query] as const;
}
