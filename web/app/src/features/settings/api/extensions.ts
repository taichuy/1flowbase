import {
  checkConsoleExtensionUpdates,
  getConsoleExtensionCatalogEntry,
  getConsoleExtensionRiskChallenge,
  installConsoleExtension,
  listConsoleExtensionCatalog,
  listConsoleInstalledExtensions,
  uploadConsoleExtension,
  type ConsoleExtensionCatalogEntry,
  type ConsoleExtensionCategory,
  type ConsoleExtensionCompatibilityOverride,
  type ConsoleExtensionRiskOverride,
  type ConsoleExtensionUploadMetadata,
  type ConsoleInstalledExtension
} from '@1flowbase/api-client';

export type SettingsExtensionCategory = ConsoleExtensionCategory;
export type SettingsInstalledExtension = ConsoleInstalledExtension;
export type SettingsExtensionCatalogEntry = ConsoleExtensionCatalogEntry;

export const settingsInstalledExtensionsQueryKey = (
  cursor: string | undefined
) => ['settings', 'extension-center', 'installed', cursor ?? 'start'] as const;

export const settingsExtensionCatalogQueryKey = (
  category: SettingsExtensionCategory,
  cursor: string | undefined
) =>
  [
    'settings',
    'extension-center',
    'catalog',
    category,
    cursor ?? 'start'
  ] as const;

export function fetchSettingsInstalledExtensions(cursor?: string) {
  return listConsoleInstalledExtensions(cursor);
}

export function fetchSettingsExtensionCatalog(
  category: SettingsExtensionCategory,
  cursor?: string
) {
  return listConsoleExtensionCatalog(category, cursor);
}

export function fetchSettingsExtensionCatalogEntry(
  category: SettingsExtensionCategory,
  artifactId: string
) {
  return getConsoleExtensionCatalogEntry(category, artifactId);
}

export function checkSettingsExtensionUpdates(
  input: Parameters<typeof checkConsoleExtensionUpdates>[0],
  csrfToken: string
) {
  return checkConsoleExtensionUpdates(input, csrfToken);
}

export function installSettingsExtension(
  entry: SettingsExtensionCatalogEntry,
  csrfToken: string,
  overrides: {
    compatibility_override?: ConsoleExtensionCompatibilityOverride;
    risk_override?: ConsoleExtensionRiskOverride;
  },
  update: boolean
) {
  return installConsoleExtension(
    {
      category: entry.category,
      artifact_id: entry.id,
      ...overrides
    },
    csrfToken,
    update
  );
}

export function uploadSettingsExtension(
  file: File,
  metadata: ConsoleExtensionUploadMetadata,
  csrfToken: string,
  overrides: Parameters<typeof uploadConsoleExtension>[3] = {}
) {
  return uploadConsoleExtension(file, metadata, csrfToken, overrides);
}

export { getConsoleExtensionRiskChallenge as getSettingsExtensionRiskChallenge };
