import { apiFetch } from '../../transport';

import type {
  DeleteCustomI18nCatalogKeyRequest,
  GetI18nCatalogEntryRequest,
  I18nCatalogEntryMutationResponse,
  I18nCatalogManagementEntry,
  I18nCatalogManagementPage,
  I18nCatalogRevisionResponse,
  ListI18nCatalogEntriesRequest,
  RestoreAllI18nCatalogOverridesRequest,
  RestoreI18nCatalogOverrideRequest,
  UpsertI18nCatalogTranslationRequest
} from './types';

const MANAGEMENT_BASE_PATH = '/api/console/settings/i18n';

export function listI18nCatalogEntries(
  request: ListI18nCatalogEntriesRequest = {},
  baseUrl?: string
): Promise<I18nCatalogManagementPage> {
  const query = new URLSearchParams();
  if (request.module !== undefined) {
    query.set('module', request.module);
  }
  if (request.locale !== undefined) {
    query.set('locale', request.locale);
  }
  if (request.search !== undefined) {
    query.set('search', request.search);
  }
  if (request.origin !== undefined) {
    query.set('origin', request.origin);
  }
  if (request.offset !== undefined) {
    query.set('offset', String(request.offset));
  }
  if (request.limit !== undefined) {
    query.set('limit', String(request.limit));
  }
  const queryString = query.toString();
  const suffix = queryString.length > 0 ? `?${queryString}` : '';

  return apiFetch<I18nCatalogManagementPage>({
    path: `${MANAGEMENT_BASE_PATH}/entries${suffix}`,
    baseUrl
  });
}

export function getI18nCatalogEntry(
  request: GetI18nCatalogEntryRequest,
  baseUrl?: string
): Promise<I18nCatalogManagementEntry> {
  const query = new URLSearchParams({
    module: request.module,
    msgid: request.msgid,
    locale: request.locale
  });
  return apiFetch<I18nCatalogManagementEntry>({
    path: `${MANAGEMENT_BASE_PATH}/entries/detail?${query.toString()}`,
    baseUrl
  });
}

export function upsertI18nCatalogOverride(
  request: UpsertI18nCatalogTranslationRequest,
  csrfToken: string,
  baseUrl?: string
): Promise<I18nCatalogEntryMutationResponse> {
  return apiFetch<I18nCatalogEntryMutationResponse>({
    path: `${MANAGEMENT_BASE_PATH}/overrides`,
    method: 'PUT',
    body: request,
    csrfToken,
    baseUrl
  });
}

export function restoreI18nCatalogOverride(
  request: RestoreI18nCatalogOverrideRequest,
  csrfToken: string,
  baseUrl?: string
): Promise<I18nCatalogEntryMutationResponse> {
  return apiFetch<I18nCatalogEntryMutationResponse>({
    path: `${MANAGEMENT_BASE_PATH}/overrides`,
    method: 'DELETE',
    body: request,
    csrfToken,
    baseUrl
  });
}

export function upsertCustomI18nCatalogTranslation(
  request: UpsertI18nCatalogTranslationRequest,
  csrfToken: string,
  baseUrl?: string
): Promise<I18nCatalogEntryMutationResponse> {
  return apiFetch<I18nCatalogEntryMutationResponse>({
    path: `${MANAGEMENT_BASE_PATH}/custom-translations`,
    method: 'PUT',
    body: request,
    csrfToken,
    baseUrl
  });
}

export function deleteCustomI18nCatalogKey(
  request: DeleteCustomI18nCatalogKeyRequest,
  csrfToken: string,
  baseUrl?: string
): Promise<I18nCatalogRevisionResponse> {
  return apiFetch<I18nCatalogRevisionResponse>({
    path: `${MANAGEMENT_BASE_PATH}/custom-keys`,
    method: 'DELETE',
    body: request,
    csrfToken,
    baseUrl
  });
}

export function restoreAllI18nCatalogOverrides(
  request: RestoreAllI18nCatalogOverridesRequest,
  csrfToken: string,
  baseUrl?: string
): Promise<I18nCatalogRevisionResponse> {
  return apiFetch<I18nCatalogRevisionResponse>({
    path: `${MANAGEMENT_BASE_PATH}/restore-overrides`,
    method: 'POST',
    body: request,
    csrfToken,
    baseUrl
  });
}
