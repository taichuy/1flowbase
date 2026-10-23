import {
  getI18nCatalogState,
  getI18nCatalogEntry,
  getI18nCatalogUpdateStatus,
  listI18nCatalogEntries,
  type GetI18nCatalogEntryRequest,
  type I18nCatalogManagementEntry,
  type I18nCatalogManagementOrigin,
  type ListI18nCatalogEntriesRequest
} from '@1flowbase/api-client';

export {
  activateI18nCatalogUpdate as activateSettingsI18nCatalogUpdate,
  previewInstalledI18nCatalog as previewSettingsInstalledI18nCatalog,
  activateInstalledI18nCatalog as activateSettingsInstalledI18nCatalog,
  upsertI18nCatalogOverride as saveSettingsI18nCatalogOverride,
  upsertCustomI18nCatalogTranslation as saveSettingsCustomI18nCatalogTranslation,
  restoreI18nCatalogOverride as restoreSettingsI18nCatalogOverride,
  deleteCustomI18nCatalogKey as deleteSettingsCustomI18nCatalogKey,
  restoreAllI18nCatalogOverrides as restoreAllSettingsI18nCatalogOverrides
} from '@1flowbase/api-client';

export type SettingsI18nCatalogEntry = I18nCatalogManagementEntry;
export type SettingsI18nCatalogOrigin = I18nCatalogManagementOrigin;
export type SettingsI18nCatalogListRequest = ListI18nCatalogEntriesRequest;

export const settingsI18nCatalogQueryKey = [
  'settings',
  'i18n-catalog'
] as const;

export function settingsI18nCatalogListQueryKey(
  request: SettingsI18nCatalogListRequest
) {
  return [...settingsI18nCatalogQueryKey, 'list', request] as const;
}

export function settingsI18nCatalogEntryQueryKey(
  request: GetI18nCatalogEntryRequest
) {
  return [...settingsI18nCatalogQueryKey, 'entry', request] as const;
}

export const fetchSettingsI18nCatalogEntries = listI18nCatalogEntries;
export const fetchSettingsI18nCatalogEntry = getI18nCatalogEntry;
export const fetchSettingsI18nCatalogState = getI18nCatalogState;
export const fetchSettingsI18nCatalogUpdateStatus = getI18nCatalogUpdateStatus;
