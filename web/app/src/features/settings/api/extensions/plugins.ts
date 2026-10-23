import {
  installConsoleOfficialPlugin,
  listConsolePluginFamilies,
  listConsoleOfficialPluginCatalog,
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
import { lookupCatalogText } from '../catalog/localized-text';

export {
  uploadConsolePluginPackage as uploadSettingsPluginPackage,
  refreshConsolePluginCurrentNodeArtifact as refreshSettingsPluginCurrentNodeArtifact,
  installConsolePluginCurrentNodeArtifact as installSettingsPluginCurrentNodeArtifact,
  deleteConsolePluginFamily as deleteSettingsPluginFamily,
  getConsolePluginTask as fetchSettingsPluginTask
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
  return (
    lookupCatalogText(response, entry.namespace, [
      entry.provider_label_key,
      entry.label_key
    ]) ??
    entry.provider_code ??
    entry.plugin_id ??
    entry.namespace
  );
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
  return entry.description_key
    ? lookupCatalogText(response, entry.namespace, [entry.description_key])
    : null;
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
