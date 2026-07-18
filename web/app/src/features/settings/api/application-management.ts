import {
  listConsoleApplicationManagement,
  type ConsoleApplicationManagementItem,
  type ConsoleApplicationManagementPage,
  type ConsoleApplicationManagementQuery
} from '@1flowbase/api-client';

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

export function fetchSettingsApplicationManagement(
  query: SettingsApplicationManagementQuery
): Promise<SettingsApplicationManagementPage> {
  return listConsoleApplicationManagement(query);
}

export async function fetchAllSettingsApplicationManagement(
  query: Omit<SettingsApplicationManagementQuery, 'page' | 'page_size'>
): Promise<SettingsApplicationManagementItem[]> {
  const pageSize = 100;
  const items: SettingsApplicationManagementItem[] = [];
  let page = 1;

  while (true) {
    const result = await listConsoleApplicationManagement({
      ...query,
      page,
      page_size: pageSize
    });
    items.push(...result.items);
    if (items.length >= result.total || result.items.length === 0) {
      return items;
    }
    page += 1;
  }
}
