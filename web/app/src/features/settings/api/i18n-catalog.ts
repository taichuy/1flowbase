import {
  activateI18nCatalogUpdate,
  activateInstalledI18nCatalog,
  deleteCustomI18nCatalogKey,
  getI18nCatalogState,
  getI18nCatalogEntry,
  getI18nCatalogUpdateStatus,
  listI18nCatalogEntries,
  previewInstalledI18nCatalog,
  restoreAllI18nCatalogOverrides,
  restoreI18nCatalogOverride,
  upsertCustomI18nCatalogTranslation,
  upsertI18nCatalogOverride,
  type DeleteCustomI18nCatalogKeyRequest,
  type GetI18nCatalogEntryRequest,
  type I18nCatalogManagementEntry,
  type I18nCatalogManagementOrigin,
  type ActivateInstalledI18nCatalogRequest,
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
export const fetchSettingsI18nCatalogState = getI18nCatalogState;
export const fetchSettingsI18nCatalogUpdateStatus = getI18nCatalogUpdateStatus;

export function activateSettingsI18nCatalogUpdate(
  request: Parameters<typeof activateI18nCatalogUpdate>[0],
  csrfToken: string
) {
  return activateI18nCatalogUpdate(request, csrfToken);
}

export function previewSettingsInstalledI18nCatalog(
  extensionInstallationId: string
) {
  return previewInstalledI18nCatalog(extensionInstallationId);
}

export function activateSettingsInstalledI18nCatalog(
  extensionInstallationId: string,
  request: ActivateInstalledI18nCatalogRequest,
  csrfToken: string
) {
  return activateInstalledI18nCatalog(
    extensionInstallationId,
    request,
    csrfToken
  );
}

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
