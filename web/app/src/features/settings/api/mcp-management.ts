import type { QueryClient } from '@tanstack/react-query';
import {
  exportConsoleMcpCatalog,
  fetchConsoleMcpBundleExportDefaults,
  fetchConsoleOfficialMcpBundles,
  fetchConsoleMcpCatalog,
  fetchConsoleMcpInstances,
  fetchConsoleMcpInstancesPage,
  fetchConsoleMcpToolsPage,
  fetchConsoleMcpTool,
  fetchConsoleMcpInterfaceCapabilities,
  fetchConsoleMcpInterfaceCapabilitiesPage,
  fetchConsoleMcpInterfaceCapability,
  fetchConsoleMcpTemplateLibrary,
  refreshConsoleMcpTemplateLibrary,
  fetchConsoleMcpUpstreamConnections,
  moveConsoleMcpGroup,
  type ConsoleMcpCatalog,
  type ConsoleMcpInstance,
  type ConsoleMcpInstancePage,
  type ConsoleMcpInstancesPageQuery,
  type ConsoleMcpInstancesQuery,
  type ConsoleMcpToolSummary,
  type ConsoleMcpToolSummaryPage,
  type ConsoleMcpToolsPageQuery,
  type ConsoleMcpInterfaceCapability,
  type ConsoleMcpInterfaceCapabilitySummary,
  type ConsoleMcpInterfaceCapabilityPage,
  type ConsoleMcpInterfaceCapabilitiesPageQuery,
  type ConsoleMcpBundleExportDefaults,
  type ConsoleMcpToolDebugExecuteResponse,
  type ExecuteConsoleMcpToolDebugBody
} from '@1flowbase/api-client';

export {
  fetchConsoleMcpInstanceDirectory as fetchSettingsMcpInstanceDirectory,
  replaceConsoleMcpInstanceDirectory as replaceSettingsMcpInstanceDirectory,
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
  ConsoleMcpInstanceDirectory,
  ReplaceConsoleMcpInstanceDirectoryBody,
  ConsoleMcpInstanceDirectory as SettingsMcpInstanceDirectory,
  ReplaceConsoleMcpInstanceDirectoryBody as ReplaceSettingsMcpInstanceDirectoryBody,
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
export type SettingsMcpInstance = ConsoleMcpInstance;
export type SettingsMcpInstancePage = ConsoleMcpInstancePage;
export type SettingsMcpInstancesQuery = ConsoleMcpInstancesQuery;
export type SettingsMcpInstancesPageQuery = ConsoleMcpInstancesPageQuery;
export type SettingsMcpToolSummary = ConsoleMcpToolSummary;
export type SettingsMcpToolSummaryPage = ConsoleMcpToolSummaryPage;
export type SettingsMcpToolsPageQuery = ConsoleMcpToolsPageQuery;
export type SettingsMcpInterfaceCapability = ConsoleMcpInterfaceCapability;
export type SettingsMcpInterfaceCapabilitySummary =
  ConsoleMcpInterfaceCapabilitySummary;
export type SettingsMcpInterfaceCapabilityPage =
  ConsoleMcpInterfaceCapabilityPage;
export type SettingsMcpInterfaceCapabilitiesPageQuery =
  ConsoleMcpInterfaceCapabilitiesPageQuery;
export type ExecuteSettingsMcpToolDebugBody = ExecuteConsoleMcpToolDebugBody;
export type SettingsMcpToolDebugExecuteResponse =
  ConsoleMcpToolDebugExecuteResponse;
export type SettingsMcpBundleExportDefaults = ConsoleMcpBundleExportDefaults;

export const settingsMcpCatalogQueryKey = [
  'settings',
  'mcp-management',
  'catalog'
] as const;

export const settingsMcpInstancesQueryKey = [
  'settings',
  'mcp-management',
  'instances'
] as const;

export const settingsMcpToolsQueryKey = [
  'settings',
  'mcp-management',
  'tools'
] as const;

export function settingsMcpToolDetailQueryKey(toolId: string) {
  return ['settings', 'mcp-management', 'tools', 'detail', toolId] as const;
}

export function settingsMcpInstanceDiscoveryPolicyQueryKey(instanceId: string) {
  return [
    'settings',
    'mcp-management',
    'discovery-policy',
    instanceId
  ] as const;
}

export const settingsMcpInterfaceCapabilitiesQueryKey = [
  'settings',
  'mcp-management',
  'interface-capabilities'
] as const;

export function settingsMcpInterfaceCapabilitiesPageQueryKey(
  query: SettingsMcpInterfaceCapabilitiesPageQuery
) {
  return [
    ...settingsMcpInterfaceCapabilitiesQueryKey,
    'page',
    query
  ] as const;
}

export function settingsMcpInterfaceCapabilityDetailQueryKey(
  interfaceId: string
) {
  return [
    ...settingsMcpInterfaceCapabilitiesQueryKey,
    'detail',
    interfaceId
  ] as const;
}

export function invalidateSettingsMcpInstancesAndTools(
  queryClient: QueryClient
) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: settingsMcpInstancesQueryKey }),
    queryClient.invalidateQueries({ queryKey: settingsMcpToolsQueryKey }),
    queryClient.invalidateQueries({ queryKey: settingsMcpCatalogQueryKey })
  ]);
}

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

export function fetchSettingsMcpInstances(
  options?: SettingsMcpInstancesQuery
) {
  return fetchConsoleMcpInstances(options);
}

export function fetchSettingsMcpInstancesPage(
  options?: SettingsMcpInstancesPageQuery
) {
  return fetchConsoleMcpInstancesPage(options);
}

export function fetchSettingsMcpToolsPage(options?: SettingsMcpToolsPageQuery) {
  return fetchConsoleMcpToolsPage(options);
}

export function fetchSettingsMcpTool(toolId: string) {
  return fetchConsoleMcpTool(toolId);
}

export function fetchSettingsMcpInterfaceCapabilities() {
  return fetchConsoleMcpInterfaceCapabilities({ bindable_only: false });
}

export function fetchSettingsMcpInterfaceCapabilitiesPage(
  options?: SettingsMcpInterfaceCapabilitiesPageQuery
) {
  return fetchConsoleMcpInterfaceCapabilitiesPage({
    bindable_only: false,
    ...options
  });
}

export function fetchSettingsMcpInterfaceCapability(interfaceId: string) {
  return fetchConsoleMcpInterfaceCapability(interfaceId);
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
