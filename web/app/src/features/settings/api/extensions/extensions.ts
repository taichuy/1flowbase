import {
  applyConsoleInstalledMcpExtension,
  getConsoleInstalledMcpExtensionConflict,
  getConsoleInstalledMcpExtensionIntegrityChallenge,
  getConsoleExtensionRiskChallenge,
  installConsoleExtension,
  uploadConsoleExtension,
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

export {
  getConsoleExtensionCatalogEntry as fetchSettingsExtensionCatalogEntry,
  checkConsoleExtensionUpdates as checkSettingsExtensionUpdates,
  deleteConsoleInstalledExtension as deleteSettingsInstalledExtension,
  enableConsoleInstalledExtension as enableSettingsInstalledExtension,
  disableConsoleInstalledExtension as disableSettingsInstalledExtension,
  previewConsoleBuiltinMcpTemplate as previewSettingsBuiltinMcpTemplate,
  applyConsoleBuiltinMcpTemplate as applySettingsBuiltinMcpTemplate
} from '@1flowbase/api-client';

export type SettingsExtensionCategory = ConsoleExtensionCategory;
export type SettingsExtensionApplicationAction =
  ConsoleExtensionApplicationAction;
export type SettingsExtensionCenterCategory =
  | 'installed'
  | 'model-pricing'
  | 'ui-components'
  | 'application-templates'
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
  csrfToken: string,
  instanceId?: string
) {
  return previewConsoleInstalledMcpExtension(
    extensionInstallationId,
    csrfToken,
    instanceId ? { instance_id: instanceId } : {}
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
export const uploadSettingsExtension = uploadConsoleExtension;
export { getConsoleInstalledMcpExtensionConflict as getSettingsInstalledMcpExtensionConflict };
export { getConsoleInstalledMcpExtensionIntegrityChallenge as getSettingsInstalledMcpExtensionIntegrityChallenge };
