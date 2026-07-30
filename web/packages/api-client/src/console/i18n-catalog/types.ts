export type I18nCatalogManagementOrigin =
  | 'official'
  | 'official_override'
  | 'custom'
  | 'english';

export interface I18nCatalogManagementEntry {
  key: string;
  locale: string;
  official_translation: string | null;
  override_translation: string | null;
  custom_translation: string | null;
  effective_value: string;
  origin: I18nCatalogManagementOrigin;
  missing: boolean;
  obsolete: boolean;
  revision: number;
}

export interface I18nCatalogManagementPage {
  entries: I18nCatalogManagementEntry[];
  total: number;
  revision: number;
}

export interface ListI18nCatalogEntriesRequest {
  locale?: string;
  search?: string;
  origin?: I18nCatalogManagementOrigin;
  offset?: number;
  limit?: number;
}

export interface GetI18nCatalogEntryRequest {
  key: string;
  locale: string;
}

export interface UpsertI18nCatalogTranslationRequest {
  key: string;
  locale: string;
  translation: string;
  expected_revision: number;
}

export interface RestoreI18nCatalogOverrideRequest {
  key: string;
  locale: string;
  expected_revision: number;
}

export interface DeleteCustomI18nCatalogKeyRequest {
  key: string;
  expected_revision: number;
}

export interface RestoreAllI18nCatalogOverridesRequest {
  expected_revision: number;
}

export interface I18nCatalogEntryMutationResponse {
  revision: number;
  entry: I18nCatalogManagementEntry;
}

export interface I18nCatalogRevisionResponse {
  revision: number;
}

export interface RuntimeI18nCatalog {
  catalog_revision: number;
  locale: string;
  digest: string;
  messages: Record<string, string>;
}

export interface GetRuntimeI18nCatalogRequest {
  locale: string;
  ifNoneMatch?: string;
}

export type ConditionalI18nCatalogResponse<T> =
  | { kind: 'ok'; value: T; etag: string | null }
  | { kind: 'not_modified'; etag: string | null };
