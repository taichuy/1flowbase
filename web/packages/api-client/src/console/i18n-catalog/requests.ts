import { apiFetch, getDefaultApiBaseUrl } from '../../transport';
import { ApiClientError } from '../../errors';

import type {
  DeleteCustomI18nCatalogKeyRequest,
  GetI18nCatalogEntryRequest,
  GetRuntimeI18nBundleRequest,
  GetRuntimeI18nManifestRequest,
  I18nCatalogEntryMutationResponse,
  I18nCatalogManagementEntry,
  I18nCatalogManagementPage,
  I18nCatalogRevisionResponse,
  ConditionalI18nCatalogResponse,
  ListI18nCatalogEntriesRequest,
  RestoreAllI18nCatalogOverridesRequest,
  RestoreI18nCatalogOverrideRequest,
  RuntimeI18nBundle,
  RuntimeI18nManifest,
  UpsertI18nCatalogTranslationRequest
} from './types';

const MANAGEMENT_BASE_PATH = '/api/console/settings/i18n';
const RUNTIME_BASE_PATH = '/api/console/i18n';

async function fetchConditionalCatalog<T>(
  path: string,
  ifNoneMatch: string | undefined,
  baseUrl = getDefaultApiBaseUrl()
): Promise<ConditionalI18nCatalogResponse<T>> {
  const response = await fetch(`${baseUrl}${path}`, {
    credentials: 'include',
    headers: ifNoneMatch ? { 'if-none-match': ifNoneMatch } : undefined
  });
  const etag = response.headers.get('etag');
  if (response.status === 304) {
    return { kind: 'not_modified', etag };
  }
  if (!response.ok) {
    throw await ApiClientError.fromResponse(response);
  }
  return { kind: 'ok', value: (await response.json()) as T, etag };
}

export function getRuntimeI18nManifest(
  request: GetRuntimeI18nManifestRequest,
  baseUrl?: string
): Promise<ConditionalI18nCatalogResponse<RuntimeI18nManifest>> {
  const query = new URLSearchParams({ locale: request.locale });
  return fetchConditionalCatalog(
    `${RUNTIME_BASE_PATH}/manifest?${query.toString()}`,
    request.ifNoneMatch,
    baseUrl
  );
}

export function getRuntimeI18nBundle(
  request: GetRuntimeI18nBundleRequest,
  baseUrl?: string
): Promise<ConditionalI18nCatalogResponse<RuntimeI18nBundle>> {
  const query = new URLSearchParams({
    module: request.module,
    locale: request.locale
  });
  return fetchConditionalCatalog(
    `${RUNTIME_BASE_PATH}/bundles/${encodeURIComponent(request.digest)}?${query.toString()}`,
    request.ifNoneMatch,
    baseUrl
  );
}

export function listI18nCatalogEntries(
  request: ListI18nCatalogEntriesRequest = {},
  baseUrl?: string
): Promise<I18nCatalogManagementPage> {
  const query = new URLSearchParams();
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
    key: request.key,
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
