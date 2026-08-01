import {
  activateConsoleInstalledI18nExtension,
  applyConsoleInstalledMcpExtension,
  checkConsoleExtensionUpdates,
  getConsoleExtensionCatalogEntry,
  getConsoleInstalledMcpExtensionConflict,
  getConsoleInstalledMcpExtensionIntegrityChallenge,
  getConsoleExtensionRiskChallenge,
  installConsoleExtension,
  listConsoleExtensionCatalog,
  listConsoleInstalledExtensions,
  previewConsoleInstalledMcpExtension,
  previewConsoleInstalledI18nExtension,
  type ConsoleExtensionCatalogEntry,
  type ConsoleExtensionApplicationAction,
  type ConsoleExtensionCategory,
  type ConsoleExtensionCompatibilityOverride,
  type ConsoleExtensionRiskOverride,
  type ConsoleInstalledMcpExtensionApplyOptions,
  type ConsoleInstalledExtension
} from '@1flowbase/api-client';

export type SettingsExtensionCategory = ConsoleExtensionCategory;
export type SettingsExtensionApplicationAction =
  ConsoleExtensionApplicationAction;
export type SettingsExtensionCenterCategory =
  | 'installed'
  | SettingsExtensionCategory;
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
  catalogId: string
) {
  return getConsoleExtensionCatalogEntry(category, catalogId);
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
      catalog_id: entry.id,
      version: entry.version,
      ...overrides
    },
    csrfToken,
    update
  );
}

export function previewSettingsInstalledMcpExtension(
  extensionInstallationId: string,
  csrfToken: string
) {
  return previewConsoleInstalledMcpExtension(
    extensionInstallationId,
    csrfToken
  );
}

export function applySettingsInstalledMcpExtension(
  extensionInstallationId: string,
  csrfToken: string,
  options: ConsoleInstalledMcpExtensionApplyOptions = {}
) {
  return applyConsoleInstalledMcpExtension(
    extensionInstallationId,
    csrfToken,
    options
  );
}

export function previewSettingsInstalledI18nExtension(
  extensionInstallationId: string
) {
  return previewConsoleInstalledI18nExtension(extensionInstallationId);
}

export function activateSettingsInstalledI18nExtension(
  extensionInstallationId: string,
  input: Parameters<typeof activateConsoleInstalledI18nExtension>[1],
  csrfToken: string
) {
  return activateConsoleInstalledI18nExtension(
    extensionInstallationId,
    input,
    csrfToken
  );
}

export { getConsoleExtensionRiskChallenge as getSettingsExtensionRiskChallenge };
export { getConsoleInstalledMcpExtensionConflict as getSettingsInstalledMcpExtensionConflict };
export { getConsoleInstalledMcpExtensionIntegrityChallenge as getSettingsInstalledMcpExtensionIntegrityChallenge };
