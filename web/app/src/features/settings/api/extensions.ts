import {
  applyConsoleInstalledMcpExtension,
  checkConsoleExtensionUpdates,
  deleteConsoleInstalledExtension,
  getConsoleExtensionCatalogEntry,
  getConsoleInstalledMcpExtensionConflict,
  getConsoleInstalledMcpExtensionIntegrityChallenge,
  getConsoleExtensionRiskChallenge,
  installConsoleExtension,
  listConsoleExtensionCatalog,
  listConsoleInstalledExtensions,
  previewConsoleInstalledMcpExtension,
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
  cursor: string | undefined,
  category?: SettingsExtensionCategory
) =>
  [
    'settings',
    'extension-center',
    'installed',
    category ?? 'all',
    cursor ?? 'start'
  ] as const;

export const settingsExtensionCatalogQueryKey = (
  category: SettingsExtensionCategory,
  {
    q,
    slot_code,
    cursor
  }: {
    q?: string;
    slot_code?: string;
    cursor?: string;
  }
) =>
  [
    'settings',
    'extension-center',
    'catalog',
    category,
    q ?? '',
    slot_code ?? 'all-slots',
    cursor ?? 'start'
  ] as const;

export function fetchSettingsInstalledExtensions(
  cursor?: string,
  category?: SettingsExtensionCategory
) {
  return listConsoleInstalledExtensions(cursor, 20, category);
}

export function fetchSettingsExtensionCatalog(
  category: SettingsExtensionCategory,
  query: {
    q?: string;
    slot_code?: string;
    cursor?: string;
  } = {}
) {
  return listConsoleExtensionCatalog(category, query);
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

export function deleteSettingsInstalledExtension(
  installationId: string,
  csrfToken: string
) {
  return deleteConsoleInstalledExtension(installationId, csrfToken);
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

export { getConsoleExtensionRiskChallenge as getSettingsExtensionRiskChallenge };
export { getConsoleInstalledMcpExtensionConflict as getSettingsInstalledMcpExtensionConflict };
export { getConsoleInstalledMcpExtensionIntegrityChallenge as getSettingsInstalledMcpExtensionIntegrityChallenge };
