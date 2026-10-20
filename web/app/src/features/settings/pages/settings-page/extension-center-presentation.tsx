

import { installSettingsExtension, type SettingsExtensionCatalogEntry, type SettingsExtensionCategory, type SettingsInstalledExtension } from '../../api/extensions/extensions';

export type ExtensionRow =
  | SettingsInstalledExtension
  | SettingsExtensionCatalogEntry;

export type UpdateState =
  | 'unchecked'
  | 'checking'
  | 'current'
  | 'update_available'
  | 'unknown_error';

export type ExtensionOperation = {
  kind: 'catalog';
  entry: SettingsExtensionCatalogEntry;
  update: boolean;
  activateI18nAfterInstall?: boolean;
};

export type ExtensionOverrides = Parameters<typeof installSettingsExtension>[2];

export const CATEGORIES: SettingsExtensionCategory[] = [
  'agent-flow',
  'capability-plugins',
  'host-extensions',
  'i18n',
  'mcp',
  'runtime-extensions'
];

export function isInstalledRow(
  row: ExtensionRow
): row is SettingsInstalledExtension {
  return 'node_id' in row;
}

export function supportsFamilyUninstall(category: SettingsExtensionCategory) {
  return category === 'runtime-extensions' || category === 'capability-plugins';
}

export function isFamilyUninstallRow(
  row: ExtensionRow
): row is SettingsInstalledExtension {
  return isInstalledRow(row) && supportsFamilyUninstall(row.category);
}

export function extensionCatalogId(row: ExtensionRow) {
  return isInstalledRow(row) ? row.catalog_id : row.id;
}

export function extensionName(row: ExtensionRow) {
  return isInstalledRow(row) ? row.artifact_id : row.name;
}

export function extensionVersion(row: ExtensionRow) {
  return row.version;
}

export function extensionHostRequirement(row: ExtensionRow) {
  return isInstalledRow(row) ? null : row.host_version_requirement;
}

export function extensionSource(row: ExtensionRow) {
  return isInstalledRow(row) ? row.source_kind : row.catalog_source;
}

export function extensionDescription(row: ExtensionRow) {
  return isInstalledRow(row) ? null : row.description;
}

function extensionInstallationStatus(row: ExtensionRow) {
  return isInstalledRow(row) ? row.status : row.installation_status;
}

export function extensionInstallationStatusLabel(
  row: ExtensionRow,
  t: (key: string) => string
) {
  return extensionInstallationStatus(row) === 'uninstalled'
    ? t('auto.uninstalled')
    : extensionInstallationStatus(row);
}

export function extensionApplicationStatusLabel(
  status: SettingsInstalledExtension['application_status'],
  t: (key: string) => string
) {
  switch (status) {
    case 'not_required':
      return t('auto.extension_application_not_required');
    case 'not_applied':
      return t('auto.extension_application_not_applied');
    case 'applied':
      return t('auto.extension_application_applied');
    case 'available':
      return t('auto.extension_application_available');
  }
}

export function mcpTemplateWorkspaceStatusLabel(
  status: SettingsExtensionCatalogEntry['mcp_instances'][number]['workspace_status'],
  t: (key: string) => string
) {
  switch (status) {
    case 'applied':
      return t('auto.extension_application_applied');
    case 'missing':
      return t('auto.revoked');
    case 'modified':
      return t('auto.mcp_template_instance_status_modified');
  }
}

export function extensionKey(row: ExtensionRow) {
  return extensionCatalogId(row);
}
