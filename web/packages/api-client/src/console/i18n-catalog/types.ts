export type I18nCatalogManagementOrigin =
  | 'official'
  | 'official_override'
  | 'custom'
  | 'english';

export interface I18nCatalogManagementEntry {
  module: string;
  msgid: string;
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
  module?: string;
  locale?: string;
  search?: string;
  origin?: I18nCatalogManagementOrigin;
  offset?: number;
  limit?: number;
}

export interface GetI18nCatalogEntryRequest {
  module: string;
  msgid: string;
  locale: string;
}

export interface UpsertI18nCatalogTranslationRequest {
  module: string;
  msgid: string;
  locale: string;
  translation: string;
  expected_revision: number;
}

export interface RestoreI18nCatalogOverrideRequest {
  module: string;
  msgid: string;
  locale: string;
  expected_revision: number;
}

export interface DeleteCustomI18nCatalogKeyRequest {
  module: string;
  msgid: string;
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

export interface RuntimeI18nManifestModule {
  module: string;
  digest: string;
  href: string;
}

export interface RuntimeI18nManifest {
  catalog_revision: number;
  locale: string;
  modules: RuntimeI18nManifestModule[];
}

export interface RuntimeI18nBundle {
  module: string;
  locale: string;
  messages: Record<string, string>;
}

export interface GetRuntimeI18nManifestRequest {
  locale: string;
  ifNoneMatch?: string;
}

export interface GetRuntimeI18nBundleRequest {
  module: string;
  locale: string;
  digest: string;
  ifNoneMatch?: string;
}

export type ConditionalI18nCatalogResponse<T> =
  | { kind: 'ok'; value: T; etag: string | null }
  | { kind: 'not_modified'; etag: string | null };
