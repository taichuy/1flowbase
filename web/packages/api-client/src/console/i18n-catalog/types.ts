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

export interface I18nCatalogState {
  active_catalog_version: string | null;
  revision: number;
  source: 'official';
  source_locale: string;
  locales: string[];
}

export interface I18nCatalogUpdateStatus {
  status: 'current' | 'update_available';
  active_catalog_version: string | null;
  latest_catalog_version: string;
}

export interface ActivateI18nCatalogRequest {
  expected_revision: number;
}

export interface I18nCatalogActivation {
  status: 'current' | 'activated';
  catalog_version: string;
  revision: number;
}

export interface I18nCatalogIntegrityWarning {
  code: string;
  message: string;
  overridable: boolean;
}

export interface I18nCatalogIntegrityChallenge {
  warnings: I18nCatalogIntegrityWarning[];
  compatibility: null;
}

export interface I18nCatalogIntegrityOverride {
  reason: string;
  acknowledged_warnings: string[];
}

export interface InstalledI18nCatalogPreview {
  extension_installation_id: string;
  application_status: 'not_applied' | 'applied';
  active_catalog_version: string | null;
  installed_catalog_version: string;
  revision: number;
  integrity_warnings: I18nCatalogIntegrityWarning[];
  required_integrity_override: I18nCatalogIntegrityChallenge | null;
}

export interface ActivateInstalledI18nCatalogRequest extends ActivateI18nCatalogRequest {
  integrity_override?: I18nCatalogIntegrityOverride;
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
