import {
  deleteConsolePluginFamily,
  getConsolePluginTask,
  installConsolePluginCurrentNodeArtifact,
  installConsoleOfficialPlugin,
  listConsolePluginFamilies,
  listConsoleOfficialPluginCatalog,
  refreshConsolePluginCurrentNodeArtifact,
  uploadConsolePluginPackage,
  type ConsolePluginFamilyCatalogResponse,
  type ConsoleOfficialPluginCatalogResponse,
  switchConsolePluginFamilyVersion,
  upgradeConsolePluginFamilyLatest,
  type ConsolePluginFamilyEntry,
  type ConsolePluginCompatibilityOverride,
  type ConsoleOfficialPluginCatalogEntry,
  type ConsolePluginInstallation,
  type InstallConsolePluginResult,
  type ConsolePluginTask
} from '@1flowbase/api-client';

export type SettingsPluginFamilyEntry = ConsolePluginFamilyEntry & {
  display_name: string;
  description: string | null;
};
export type SettingsOfficialPluginCatalogEntry =
  ConsoleOfficialPluginCatalogEntry;
export type SettingsPluginCompatibilityOverride =
  ConsolePluginCompatibilityOverride;
export type SettingsOfficialPluginCatalogResponse =
  ConsoleOfficialPluginCatalogResponse;
export type SettingsPluginInstallation = ConsolePluginInstallation;
export type SettingsInstallPluginResult = InstallConsolePluginResult;
export type SettingsPluginTask = ConsolePluginTask;

const MODEL_PROVIDER_PLUGIN_TYPE = 'model_provider';

export const settingsPluginFamiliesQueryKey = [
  'settings',
  'plugins',
  'families'
] as const;

export const settingsOfficialPluginsQueryKey = [
  'settings',
  'plugins',
  'official-catalog'
] as const;

function pickPreferredLocales(localeMeta: Record<string, unknown>) {
  const candidates = [
    localeMeta.resolved_locale,
    localeMeta.fallback_locale,
    'zh_Hans',
    'en_US'
  ];

  return candidates.filter(
    (value, index): value is string =>
      typeof value === 'string' && candidates.indexOf(value) === index
  );
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null
    ? (value as Record<string, unknown>)
    : null;
}

function readLocalizedValue(
  bundle: Record<string, unknown>,
  dottedKey: string
): string | null {
  let current: unknown = bundle;

  for (const segment of dottedKey.split('.')) {
    current = asRecord(current)?.[segment];
    if (current === undefined) {
      return null;
    }
  }

  return typeof current === 'string' ? current : null;
}

function resolvePluginDisplayName(
  entry: {
    namespace: string;
    provider_label_key: string;
    label_key: string;
    provider_code?: string;
    plugin_id?: string;
  },
  response: Pick<
    ConsolePluginFamilyCatalogResponse,
    'locale_meta' | 'i18n_catalog'
  >
) {
  const namespaceCatalog = asRecord(response.i18n_catalog)?.[entry.namespace];
  const localeCatalog = asRecord(namespaceCatalog);

  if (localeCatalog) {
    for (const locale of pickPreferredLocales(response.locale_meta)) {
      const localizedBundle = asRecord(localeCatalog[locale]);

      if (!localizedBundle) {
        continue;
      }

      const providerLabel = readLocalizedValue(
        localizedBundle,
        entry.provider_label_key
      );
      if (providerLabel) {
        return providerLabel;
      }

      const pluginLabel = readLocalizedValue(localizedBundle, entry.label_key);
      if (pluginLabel) {
        return pluginLabel;
      }
    }

    for (const localizedBundle of Object.values(localeCatalog)) {
      const normalizedBundle = asRecord(localizedBundle);

      if (!normalizedBundle) {
        continue;
      }

      const providerLabel = readLocalizedValue(
        normalizedBundle,
        entry.provider_label_key
      );
      if (providerLabel) {
        return providerLabel;
      }

      const pluginLabel = readLocalizedValue(normalizedBundle, entry.label_key);
      if (pluginLabel) {
        return pluginLabel;
      }
    }
  }

  return entry.provider_code ?? entry.plugin_id ?? entry.namespace;
}

function resolvePluginDescription(
  entry: {
    namespace: string;
    description_key: string | null;
  },
  response: Pick<
    ConsolePluginFamilyCatalogResponse,
    'locale_meta' | 'i18n_catalog'
  >
) {
  if (!entry.description_key) {
    return null;
  }

  const namespaceCatalog = asRecord(response.i18n_catalog)?.[entry.namespace];
  const localeCatalog = asRecord(namespaceCatalog);

  if (localeCatalog) {
    for (const locale of pickPreferredLocales(response.locale_meta)) {
      const localizedBundle = asRecord(localeCatalog[locale]);

      if (!localizedBundle) {
        continue;
      }

      const description = readLocalizedValue(
        localizedBundle,
        entry.description_key
      );
      if (description) {
        return description;
      }
    }

    for (const localizedBundle of Object.values(localeCatalog)) {
      const normalizedBundle = asRecord(localizedBundle);

      if (!normalizedBundle) {
        continue;
      }

      const description = readLocalizedValue(
        normalizedBundle,
        entry.description_key
      );
      if (description) {
        return description;
      }
    }
  }

  return null;
}

export function fetchSettingsPluginFamilies(locale?: string) {
  return listConsolePluginFamilies({
    plugin_type: MODEL_PROVIDER_PLUGIN_TYPE,
    ...(locale ? { locale } : {})
  }).then((response) =>
    response.entries.map((entry) => ({
      ...entry,
      display_name: resolvePluginDisplayName(entry, response),
      description: resolvePluginDescription(entry, response)
    }))
  );
}

export function fetchSettingsOfficialPluginCatalog({
  locale,
  q,
  cursor,
  limit = 20
}: {
  locale?: string;
  q?: string;
  cursor?: string;
  limit?: number;
} = {}) {
  return listConsoleOfficialPluginCatalog({
    plugin_type: MODEL_PROVIDER_PLUGIN_TYPE,
    ...(locale ? { locale } : {}),
    ...(q ? { q } : {}),
    ...(cursor ? { cursor } : {}),
    limit
  });
}

export function installSettingsOfficialPlugin(
  plugin_id: string,
  csrfToken: string,
  compatibilityOverride?: SettingsPluginCompatibilityOverride
) {
  return installConsoleOfficialPlugin(
    {
      plugin_id,
      ...(compatibilityOverride
        ? { compatibility_override: compatibilityOverride }
        : {})
    },
    csrfToken
  );
}

export function uploadSettingsPluginPackage(file: File, csrfToken: string) {
  return uploadConsolePluginPackage(file, csrfToken);
}

export function refreshSettingsPluginCurrentNodeArtifact(
  installationId: string,
  csrfToken: string
) {
  return refreshConsolePluginCurrentNodeArtifact(installationId, csrfToken);
}

export function installSettingsPluginCurrentNodeArtifact(
  installationId: string,
  csrfToken: string
) {
  return installConsolePluginCurrentNodeArtifact(installationId, csrfToken);
}

export function upgradeSettingsPluginFamilyLatest(
  providerCode: string,
  csrfToken: string,
  compatibilityOverride?: SettingsPluginCompatibilityOverride
) {
  if (compatibilityOverride) {
    return upgradeConsolePluginFamilyLatest(providerCode, csrfToken, {
      compatibility_override: compatibilityOverride
    });
  }

  return upgradeConsolePluginFamilyLatest(providerCode, csrfToken);
}

export function switchSettingsPluginFamilyVersion(
  providerCode: string,
  installation_id: string,
  csrfToken: string
) {
  return switchConsolePluginFamilyVersion(
    providerCode,
    { installation_id },
    csrfToken
  );
}

export function deleteSettingsPluginFamily(
  providerCode: string,
  csrfToken: string
) {
  return deleteConsolePluginFamily(providerCode, csrfToken);
}

export function fetchSettingsPluginTask(taskId: string) {
  return getConsolePluginTask(taskId);
}
