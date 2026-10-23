import {
  exportConsoleMcpCatalog,
  fetchConsoleMcpBundleExportDefaults,
  fetchConsoleOfficialMcpBundles,
  fetchConsoleMcpCatalog,
  fetchConsoleMcpInterfaceCapabilities,
  fetchConsoleMcpTemplateLibrary,
  refreshConsoleMcpTemplateLibrary,
  fetchConsoleMcpUpstreamConnections,
  moveConsoleMcpGroup,
  type ConsoleMcpCatalog,
  type ConsoleMcpInterfaceCapability,
  type ConsoleMcpBundleExportDefaults,
  type ConsoleMcpToolDebugExecuteResponse,
  type ExecuteConsoleMcpToolDebugBody
} from '@1flowbase/api-client';

export {
  createConsoleMcpUpstreamConnection as createSettingsMcpUpstreamConnection,
  updateConsoleMcpUpstreamConnection as updateSettingsMcpUpstreamConnection,
  deleteConsoleMcpUpstreamConnection as deleteSettingsMcpUpstreamConnection,
  saveConsoleMcpUpstreamConnectionCredentials as saveSettingsMcpUpstreamConnectionCredentials,
  deleteConsoleMcpUpstreamConnectionCredentials as deleteSettingsMcpUpstreamConnectionCredentials,
  testConsoleMcpUpstreamConnection as testSettingsMcpUpstreamConnection,
  testConsoleMcpUpstreamConnectionDraft as testSettingsMcpUpstreamConnectionDraft,
  discoverConsoleMcpUpstreamConnection as discoverSettingsMcpUpstreamConnection,
  importConsoleMcpUpstreamTools as importSettingsMcpUpstreamTools,
  executeConsoleMcpProxyToolDebug as executeSettingsMcpProxyToolDebug,
  fetchConsoleMcpInstanceDiscoveryPolicy as fetchSettingsMcpInstanceDiscoveryPolicy,
  fetchConsoleMcpClientCredential as fetchSettingsMcpClientCredential,
  saveConsoleMcpClientCredential as saveSettingsMcpClientCredential,
  deleteConsoleMcpClientCredential as deleteSettingsMcpClientCredential,
  exportConsoleMcpBundle as exportSettingsMcpBundle,
  exportConsoleMcpInstanceBundle as exportSettingsMcpInstanceBundle,
  previewConsoleMcpBundle as previewSettingsMcpBundle,
  importConsoleMcpBundle as importSettingsMcpBundle,
  previewConsoleOfficialMcpBundle as previewSettingsOfficialMcpBundle,
  importConsoleOfficialMcpBundle as importSettingsOfficialMcpBundle,
  syncConsoleMcpTemplateLibraryBundle as syncSettingsMcpTemplateLibraryBundle,
  previewConsoleMcpTemplateLibraryBundle as previewSettingsMcpTemplateLibraryBundle,
  importConsoleMcpTemplateLibraryBundle as importSettingsMcpTemplateLibraryBundle,
  setConsoleMcpTemplateLibraryCurrentVersion as setSettingsMcpTemplateLibraryCurrentVersion,
  deleteConsoleMcpTemplateLibraryRelease as deleteSettingsMcpTemplateLibraryRelease,
  repairConsoleMcpTemplateLibraryRelease as repairSettingsMcpTemplateLibraryRelease,
  createConsoleMcpInstance as createSettingsMcpInstance,
  updateConsoleMcpInstance as updateSettingsMcpInstance,
  copyConsoleMcpInstance as copySettingsMcpInstance,
  deleteConsoleMcpInstance as deleteSettingsMcpInstance,
  upsertConsoleMcpGroup as upsertSettingsMcpGroup,
  deleteConsoleMcpGroup as deleteSettingsMcpGroup,
  createConsoleMcpTool as createSettingsMcpTool,
  updateConsoleMcpTool as updateSettingsMcpTool,
  deleteConsoleMcpTool as deleteSettingsMcpTool,
  refreshConsoleMcpToolDescription as refreshSettingsMcpToolDescription,
  executeConsoleMcpToolDebug as executeSettingsMcpToolDebug,
  createConsoleMcpToolBinding as createSettingsMcpToolBinding,
  updateConsoleMcpToolBinding as updateSettingsMcpToolBinding,
  deleteConsoleMcpToolBinding as deleteSettingsMcpToolBinding,
  updateConsoleMcpInstanceDiscoveryPolicy as updateSettingsMcpInstanceDiscoveryPolicy
} from '@1flowbase/api-client';

export type {
  ConsoleMcpBundleImportReport as SettingsMcpBundleImportReport,
  ConsoleMcpBundlePreview as SettingsMcpBundlePreview,
  ExportConsoleMcpBundleBody as ExportSettingsMcpBundleBody,
  ConsoleOfficialMcpBundleCatalog as SettingsOfficialMcpBundleCatalog,
  CopyConsoleMcpInstanceBody as CopySettingsMcpInstanceBody,
  ConsoleOfficialMcpBundleEntry as SettingsOfficialMcpBundleEntry,
  ConsoleMcpTemplateLibrary as SettingsMcpTemplateLibrary,
  ConsoleMcpTemplateLibraryBundle as SettingsMcpTemplateLibraryBundle,
  ConsoleMcpTemplateLibraryVersion as SettingsMcpTemplateLibraryVersion
} from '@1flowbase/api-client';

export type SettingsMcpCatalog = ConsoleMcpCatalog;
export type SettingsMcpInterfaceCapability = ConsoleMcpInterfaceCapability;
export type ExecuteSettingsMcpToolDebugBody = ExecuteConsoleMcpToolDebugBody;
export type SettingsMcpToolDebugExecuteResponse =
  ConsoleMcpToolDebugExecuteResponse;
export type SettingsMcpBundleExportDefaults = ConsoleMcpBundleExportDefaults;

export const settingsMcpCatalogQueryKey = [
  'settings',
  'mcp-management',
  'catalog'
] as const;

export const settingsMcpInterfaceCapabilitiesQueryKey = [
  'settings',
  'mcp-management',
  'interface-capabilities'
] as const;

export const settingsOfficialMcpBundlesQueryKey = [
  'settings',
  'mcp-management',
  'official-bundles'
] as const;

export const settingsMcpBundleExportDefaultsQueryKey = [
  'settings',
  'mcp-management',
  'bundle-export-defaults'
] as const;

export const settingsMcpTemplateLibraryQueryKey = [
  'settings',
  'mcp-management',
  'template-library'
] as const;

export const settingsMcpUpstreamConnectionsQueryKey = [
  'settings',
  'mcp-management',
  'upstream-connections'
] as const;

export function fetchSettingsMcpCatalog() {
  return fetchConsoleMcpCatalog();
}

export function fetchSettingsMcpInterfaceCapabilities() {
  return fetchConsoleMcpInterfaceCapabilities({ bindable_only: false });
}

export function fetchSettingsMcpUpstreamConnections() {
  return fetchConsoleMcpUpstreamConnections();
}

export function exportSettingsMcpCatalog() {
  return exportConsoleMcpCatalog();
}

export function fetchSettingsMcpBundleExportDefaults() {
  return fetchConsoleMcpBundleExportDefaults();
}

export function fetchSettingsOfficialMcpBundles() {
  return fetchConsoleOfficialMcpBundles();
}

export function fetchSettingsMcpTemplateLibrary() {
  return fetchConsoleMcpTemplateLibrary();
}

export function refreshSettingsMcpTemplateLibrary() {
  return refreshConsoleMcpTemplateLibrary();
}

export function moveSettingsMcpGroup(
  instanceId: string,
  sourcePath: string,
  targetParentPath: string,
  sortOrder: number,
  csrfToken: string
) {
  return moveConsoleMcpGroup(
    instanceId,
    {
      source_path: sourcePath,
      target_parent_path: targetParentPath,
      sort_order: sortOrder
    },
    csrfToken
  );
}
