import {
  ApiClientError,
  type DeleteCustomI18nCatalogKeyRequest,
  type GetI18nCatalogEntryRequest,
  type I18nCatalogEntryMutationResponse,
  type I18nCatalogManagementPage,
  type I18nCatalogRevisionResponse,
  type RestoreAllI18nCatalogOverridesRequest,
  type RestoreI18nCatalogOverrideRequest,
  type UpsertI18nCatalogTranslationRequest
} from '@1flowbase/api-client';

import type {
  SettingsI18nCatalogEntry,
  SettingsI18nCatalogListRequest
} from '../../../api/i18n-catalog';

export const settingsI18nCatalogTestNavigation = {
  route_definitions: [
    {
      route_id: 'settings.i18n',
      surface_key: 'i18n',
      path: '/settings/i18n',
      surface_kind: 'system' as const
    }
  ],
  navigation_items: [
    {
      item_id: 'i18n',
      route_id: 'settings.i18n',
      parent_item_id: 'settings',
      label_key: 'auto.translation_catalog_title',
      navigation_slot: 'settings' as const,
      order: 4
    }
  ]
};

export const settingsI18nCatalogTestLocales = ['en_US', 'zh_Hans'] as const;

const INITIAL_REVISION = 8;

const initialEntries: SettingsI18nCatalogEntry[] = [
  {
    module: '@1flowbase/common',
    msgid: 'Settings',
    locale: 'zh_Hans',
    official_translation: '设置',
    override_translation: '系统设置',
    custom_translation: null,
    effective_value: '系统设置',
    origin: 'official_override',
    missing: false,
    obsolete: true,
    revision: INITIAL_REVISION
  },
  {
    module: '@1flowbase/common',
    msgid: 'Settings',
    locale: 'en_US',
    official_translation: 'Settings',
    override_translation: null,
    custom_translation: null,
    effective_value: 'Settings',
    origin: 'official',
    missing: false,
    obsolete: false,
    revision: INITIAL_REVISION
  },
  {
    module: '@1flowbase/common',
    msgid: 'Untranslated',
    locale: 'zh_Hans',
    official_translation: null,
    override_translation: null,
    custom_translation: null,
    effective_value: 'Untranslated',
    origin: 'english',
    missing: true,
    obsolete: false,
    revision: INITIAL_REVISION
  },
  {
    module: 'workspace/custom',
    msgid: 'Greeting',
    locale: 'zh_Hans',
    official_translation: null,
    override_translation: null,
    custom_translation: '欢迎',
    effective_value: '欢迎',
    origin: 'custom',
    missing: false,
    obsolete: false,
    revision: INITIAL_REVISION
  },
  {
    module: 'workspace/custom',
    msgid: 'Greeting',
    locale: 'en_US',
    official_translation: null,
    override_translation: null,
    custom_translation: 'Welcome',
    effective_value: 'Welcome',
    origin: 'custom',
    missing: false,
    obsolete: false,
    revision: INITIAL_REVISION
  }
];

function entryMatchesIdentity(
  entry: SettingsI18nCatalogEntry,
  identity: GetI18nCatalogEntryRequest
) {
  return (
    entry.module === identity.module &&
    entry.msgid === identity.msgid &&
    entry.locale === identity.locale
  );
}

function cloneEntry(entry: SettingsI18nCatalogEntry) {
  return { ...entry };
}

export function createSettingsI18nCatalogTestServer() {
  let revision = INITIAL_REVISION;
  let entries = initialEntries.map(cloneEntry);

  const requireRevision = (expectedRevision: number) => {
    if (expectedRevision !== revision) {
      throw new ApiClientError({
        status: 409,
        message: 'i18n_catalog_revision'
      });
    }
  };

  const commitEntries = (nextEntries: SettingsI18nCatalogEntry[]) => {
    revision += 1;
    entries = nextEntries.map((entry) => ({ ...entry, revision }));
  };

  const getState = (): I18nCatalogManagementPage => ({
    entries: entries.map(cloneEntry),
    total: entries.length,
    revision
  });

  const listEntries = async (
    request: SettingsI18nCatalogListRequest = {}
  ): Promise<I18nCatalogManagementPage> => {
    const search = request.search?.trim().toLocaleLowerCase();
    const filtered = entries.filter((entry) => {
      if (request.module && entry.module !== request.module) return false;
      if (request.locale && entry.locale !== request.locale) return false;
      if (request.origin && entry.origin !== request.origin) return false;
      if (!search) return true;

      return [entry.module, entry.msgid, entry.effective_value]
        .join('\n')
        .toLocaleLowerCase()
        .includes(search);
    });
    const offset = request.offset ?? 0;
    const limit = request.limit ?? filtered.length;

    return {
      entries: filtered.slice(offset, offset + limit).map(cloneEntry),
      total: filtered.length,
      revision
    };
  };

  const listEntriesFromSearchParams = (searchParams: URLSearchParams) =>
    listEntries({
      module: searchParams.get('module') ?? undefined,
      locale: searchParams.get('locale') ?? undefined,
      search: searchParams.get('search') ?? undefined,
      origin:
        (searchParams.get(
          'origin'
        ) as SettingsI18nCatalogListRequest['origin']) ?? undefined,
      offset: Number(searchParams.get('offset') ?? 0),
      limit: Number(searchParams.get('limit') ?? 20)
    });

  const getEntry = async (identity: GetI18nCatalogEntryRequest) => {
    const entry = entries.find((candidate) =>
      entryMatchesIdentity(candidate, identity)
    );
    if (!entry) throw new Error('catalog fixture entry not found');
    return cloneEntry(entry);
  };

  const saveOverride = async (
    request: UpsertI18nCatalogTranslationRequest
  ): Promise<I18nCatalogEntryMutationResponse> => {
    requireRevision(request.expected_revision);
    commitEntries(
      entries.map(
        (entry): SettingsI18nCatalogEntry =>
          entryMatchesIdentity(entry, request)
            ? {
                ...entry,
                override_translation: request.translation,
                effective_value: request.translation,
                origin: 'official_override'
              }
            : entry
      )
    );
    return { revision, entry: await getEntry(request) };
  };

  const saveCustomTranslation = async (
    request: UpsertI18nCatalogTranslationRequest
  ): Promise<I18nCatalogEntryMutationResponse> => {
    requireRevision(request.expected_revision);
    const existing = entries.find((entry) =>
      entryMatchesIdentity(entry, request)
    );
    const updated: SettingsI18nCatalogEntry = {
      module: request.module,
      msgid: request.msgid,
      locale: request.locale,
      official_translation: null,
      override_translation: null,
      custom_translation: request.translation,
      effective_value: request.translation,
      origin: 'custom',
      missing: false,
      obsolete: false,
      revision
    };
    commitEntries(
      existing
        ? entries.map((entry) =>
            entryMatchesIdentity(entry, request) ? updated : entry
          )
        : [...entries, updated]
    );
    return { revision, entry: await getEntry(request) };
  };

  const restoreOverride = async (
    request: RestoreI18nCatalogOverrideRequest
  ): Promise<I18nCatalogEntryMutationResponse> => {
    requireRevision(request.expected_revision);
    commitEntries(
      entries.map(
        (entry): SettingsI18nCatalogEntry =>
          entryMatchesIdentity(entry, request)
            ? {
                ...entry,
                override_translation: null,
                effective_value: entry.official_translation ?? entry.msgid,
                origin: entry.official_translation ? 'official' : 'english'
              }
            : entry
      )
    );
    return { revision, entry: await getEntry(request) };
  };

  const deleteCustomKey = async (
    request: DeleteCustomI18nCatalogKeyRequest
  ): Promise<I18nCatalogRevisionResponse> => {
    requireRevision(request.expected_revision);
    commitEntries(
      entries.filter(
        (entry) =>
          entry.module !== request.module || entry.msgid !== request.msgid
      )
    );
    return { revision };
  };

  const restoreAllOverrides = async (
    request: RestoreAllI18nCatalogOverridesRequest
  ): Promise<I18nCatalogRevisionResponse> => {
    requireRevision(request.expected_revision);
    commitEntries(
      entries.map(
        (entry): SettingsI18nCatalogEntry =>
          entry.override_translation
            ? {
                ...entry,
                override_translation: null,
                effective_value: entry.official_translation ?? entry.msgid,
                origin: entry.official_translation ? 'official' : 'english'
              }
            : entry
      )
    );
    return { revision };
  };

  return {
    navigation: settingsI18nCatalogTestNavigation,
    locales: settingsI18nCatalogTestLocales,
    getState,
    listEntries,
    listEntriesFromSearchParams,
    getEntry,
    saveOverride,
    saveCustomTranslation,
    restoreOverride,
    deleteCustomKey,
    restoreAllOverrides
  };
}
