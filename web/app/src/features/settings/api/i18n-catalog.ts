import {
  deleteCustomI18nCatalogKey,
  getI18nCatalogEntry,
  listI18nCatalogEntries,
  restoreAllI18nCatalogOverrides,
  restoreI18nCatalogOverride,
  upsertCustomI18nCatalogTranslation,
  upsertI18nCatalogOverride,
  type DeleteCustomI18nCatalogKeyRequest,
  type GetI18nCatalogEntryRequest,
  type I18nCatalogManagementEntry,
  type I18nCatalogManagementOrigin,
  type ListI18nCatalogEntriesRequest,
  type RestoreAllI18nCatalogOverridesRequest,
  type RestoreI18nCatalogOverrideRequest,
  type UpsertI18nCatalogTranslationRequest
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

export function saveSettingsI18nCatalogOverride(
  request: UpsertI18nCatalogTranslationRequest,
  csrfToken: string
) {
  return upsertI18nCatalogOverride(request, csrfToken);
}

export function saveSettingsCustomI18nCatalogTranslation(
  request: UpsertI18nCatalogTranslationRequest,
  csrfToken: string
) {
  return upsertCustomI18nCatalogTranslation(request, csrfToken);
}

export function restoreSettingsI18nCatalogOverride(
  request: RestoreI18nCatalogOverrideRequest,
  csrfToken: string
) {
  return restoreI18nCatalogOverride(request, csrfToken);
}

export function deleteSettingsCustomI18nCatalogKey(
  request: DeleteCustomI18nCatalogKeyRequest,
  csrfToken: string
) {
  return deleteCustomI18nCatalogKey(request, csrfToken);
}

export function restoreAllSettingsI18nCatalogOverrides(
  request: RestoreAllI18nCatalogOverridesRequest,
  csrfToken: string
) {
  return restoreAllI18nCatalogOverrides(request, csrfToken);
}
