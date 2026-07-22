import {
  createConsoleMcpInstance,
  copyConsoleMcpInstance,
  createConsoleMcpTool,
  createConsoleMcpToolBinding,
  createConsoleMcpUpstreamConnection,
  deleteConsoleMcpClientCredential,
  deleteConsoleMcpGroup,
  deleteConsoleMcpInstance,
  deleteConsoleMcpTool,
  deleteConsoleMcpToolBinding,
  deleteConsoleMcpUpstreamConnection,
  deleteConsoleMcpUpstreamConnectionCredentials,
  discoverConsoleMcpUpstreamConnection,
  executeConsoleMcpProxyToolDebug,
  executeConsoleMcpToolDebug,
  exportConsoleMcpBundle,
  exportConsoleMcpCatalog,
  exportConsoleMcpInstanceBundle,
  fetchConsoleOfficialMcpBundles,
  fetchConsoleMcpCatalog,
  fetchConsoleMcpClientCredential,
  fetchConsoleMcpInstanceDiscoveryPolicy,
  fetchConsoleMcpInterfaceCapabilities,
  fetchConsoleMcpUpstreamConnections,
  importConsoleMcpBundle,
  importConsoleOfficialMcpBundle,
  importConsoleMcpUpstreamTools,
  moveConsoleMcpGroup,
  previewConsoleMcpBundle,
  previewConsoleOfficialMcpBundle,
  refreshConsoleMcpToolDescription,
  saveConsoleMcpClientCredential,
  saveConsoleMcpUpstreamConnectionCredentials,
  testConsoleMcpUpstreamConnection,
  testConsoleMcpUpstreamConnectionDraft,
  updateConsoleMcpInstance,
  updateConsoleMcpInstanceDiscoveryPolicy,
  updateConsoleMcpTool,
  updateConsoleMcpToolBinding,
  updateConsoleMcpUpstreamConnection,
  upsertConsoleMcpGroup,
  type ConsoleMcpCatalog,
  type CopyConsoleMcpInstanceBody,
  type ConsoleMcpInterfaceCapability,
  type ConsoleOfficialMcpBundleBody,
  type ConsoleMcpToolDebugExecuteResponse,
  type ExecuteConsoleMcpProxyToolDebugBody,
  type ExecuteConsoleMcpToolDebugBody,
  type ExportConsoleMcpBundleBody,
  type SaveConsoleMcpGroupBody,
  type SaveConsoleMcpInstanceBody,
  type SaveConsoleMcpToolBindingBody,
  type SaveConsoleMcpToolBody,
  type SaveConsoleMcpUpstreamConnectionBody,
  type SaveConsoleMcpUpstreamConnectionCredentialsBody,
  type TestConsoleMcpUpstreamConnectionDraftBody,
  type ImportConsoleMcpUpstreamToolsBody,
  type UpdateConsoleMcpInstanceDiscoveryPolicyBody,
  type UpdateConsoleMcpToolBody
} from '@1flowbase/api-client';

export type {
  ConsoleMcpBundleImportReport as SettingsMcpBundleImportReport,
  ConsoleMcpBundlePreview as SettingsMcpBundlePreview,
  ExportConsoleMcpBundleBody as ExportSettingsMcpBundleBody,
  ConsoleOfficialMcpBundleCatalog as SettingsOfficialMcpBundleCatalog,
  CopyConsoleMcpInstanceBody as CopySettingsMcpInstanceBody,
  ConsoleOfficialMcpBundleEntry as SettingsOfficialMcpBundleEntry
} from '@1flowbase/api-client';

export type SettingsMcpCatalog = ConsoleMcpCatalog;
export type SettingsMcpInterfaceCapability = ConsoleMcpInterfaceCapability;
export type ExecuteSettingsMcpToolDebugBody = ExecuteConsoleMcpToolDebugBody;
export type SettingsMcpToolDebugExecuteResponse =
  ConsoleMcpToolDebugExecuteResponse;

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

export function createSettingsMcpUpstreamConnection(
  body: SaveConsoleMcpUpstreamConnectionBody,
  csrfToken: string
) {
  return createConsoleMcpUpstreamConnection(body, csrfToken);
}

export function updateSettingsMcpUpstreamConnection(
  connectionId: string,
  body: SaveConsoleMcpUpstreamConnectionBody,
  csrfToken: string
) {
  return updateConsoleMcpUpstreamConnection(connectionId, body, csrfToken);
}

export function deleteSettingsMcpUpstreamConnection(
  connectionId: string,
  csrfToken: string
) {
  return deleteConsoleMcpUpstreamConnection(connectionId, csrfToken);
}

export function saveSettingsMcpUpstreamConnectionCredentials(
  connectionId: string,
  body: SaveConsoleMcpUpstreamConnectionCredentialsBody,
  csrfToken: string
) {
  return saveConsoleMcpUpstreamConnectionCredentials(
    connectionId,
    body,
    csrfToken
  );
}

export function deleteSettingsMcpUpstreamConnectionCredentials(
  connectionId: string,
  csrfToken: string
) {
  return deleteConsoleMcpUpstreamConnectionCredentials(connectionId, csrfToken);
}

export function testSettingsMcpUpstreamConnection(
  connectionId: string,
  csrfToken: string
) {
  return testConsoleMcpUpstreamConnection(connectionId, csrfToken);
}

export function testSettingsMcpUpstreamConnectionDraft(
  body: TestConsoleMcpUpstreamConnectionDraftBody,
  csrfToken: string
) {
  return testConsoleMcpUpstreamConnectionDraft(body, csrfToken);
}

export function discoverSettingsMcpUpstreamConnection(
  connectionId: string,
  csrfToken: string
) {
  return discoverConsoleMcpUpstreamConnection(connectionId, csrfToken);
}

export function importSettingsMcpUpstreamTools(
  connectionId: string,
  body: ImportConsoleMcpUpstreamToolsBody,
  csrfToken: string
) {
  return importConsoleMcpUpstreamTools(connectionId, body, csrfToken);
}

export function executeSettingsMcpProxyToolDebug(
  toolId: string,
  body: ExecuteConsoleMcpProxyToolDebugBody,
  csrfToken: string
) {
  return executeConsoleMcpProxyToolDebug(toolId, body, csrfToken);
}

export function fetchSettingsMcpInstanceDiscoveryPolicy(instanceId: string) {
  return fetchConsoleMcpInstanceDiscoveryPolicy(instanceId);
}

export function fetchSettingsMcpClientCredential(instanceId: string) {
  return fetchConsoleMcpClientCredential(instanceId);
}

export function saveSettingsMcpClientCredential(
  instanceId: string,
  apiKey: string,
  csrfToken: string
) {
  return saveConsoleMcpClientCredential(instanceId, apiKey, csrfToken);
}

export function deleteSettingsMcpClientCredential(
  instanceId: string,
  csrfToken: string
) {
  return deleteConsoleMcpClientCredential(instanceId, csrfToken);
}

export function exportSettingsMcpCatalog() {
  return exportConsoleMcpCatalog();
}

export function exportSettingsMcpBundle(
  body: ExportConsoleMcpBundleBody,
  csrfToken: string
) {
  return exportConsoleMcpBundle(body, csrfToken);
}

export function exportSettingsMcpInstanceBundle(
  instanceId: string,
  body: ExportConsoleMcpBundleBody,
  csrfToken: string
) {
  return exportConsoleMcpInstanceBundle(instanceId, body, csrfToken);
}

export function previewSettingsMcpBundle(file: File, csrfToken: string) {
  return previewConsoleMcpBundle(file, csrfToken);
}

export function importSettingsMcpBundle(file: File, csrfToken: string) {
  return importConsoleMcpBundle(file, csrfToken);
}

export function fetchSettingsOfficialMcpBundles() {
  return fetchConsoleOfficialMcpBundles();
}

export function previewSettingsOfficialMcpBundle(
  body: ConsoleOfficialMcpBundleBody,
  csrfToken: string
) {
  return previewConsoleOfficialMcpBundle(body, csrfToken);
}

export function importSettingsOfficialMcpBundle(
  body: ConsoleOfficialMcpBundleBody,
  csrfToken: string
) {
  return importConsoleOfficialMcpBundle(body, csrfToken);
}

export function createSettingsMcpInstance(
  body: SaveConsoleMcpInstanceBody,
  csrfToken: string
) {
  return createConsoleMcpInstance(body, csrfToken);
}

export function updateSettingsMcpInstance(
  instanceId: string,
  body: SaveConsoleMcpInstanceBody,
  csrfToken: string
) {
  return updateConsoleMcpInstance(instanceId, body, csrfToken);
}

export function copySettingsMcpInstance(
  sourceInstanceId: string,
  body: CopyConsoleMcpInstanceBody,
  csrfToken: string
) {
  return copyConsoleMcpInstance(sourceInstanceId, body, csrfToken);
}

export function deleteSettingsMcpInstance(
  instanceId: string,
  csrfToken: string
) {
  return deleteConsoleMcpInstance(instanceId, csrfToken);
}

export function upsertSettingsMcpGroup(
  instanceId: string,
  body: SaveConsoleMcpGroupBody,
  csrfToken: string
) {
  return upsertConsoleMcpGroup(instanceId, body, csrfToken);
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

export function deleteSettingsMcpGroup(
  instanceId: string,
  path: string,
  csrfToken: string
) {
  return deleteConsoleMcpGroup(instanceId, path, csrfToken);
}

export function createSettingsMcpTool(
  body: SaveConsoleMcpToolBody,
  csrfToken: string
) {
  return createConsoleMcpTool(body, csrfToken);
}

export function updateSettingsMcpTool(
  toolId: string,
  body: UpdateConsoleMcpToolBody,
  csrfToken: string
) {
  return updateConsoleMcpTool(toolId, body, csrfToken);
}

export function deleteSettingsMcpTool(toolId: string, csrfToken: string) {
  return deleteConsoleMcpTool(toolId, csrfToken);
}

export function refreshSettingsMcpToolDescription(
  toolId: string,
  csrfToken: string
) {
  return refreshConsoleMcpToolDescription(toolId, csrfToken);
}

export function executeSettingsMcpToolDebug(
  body: ExecuteSettingsMcpToolDebugBody,
  csrfToken: string
) {
  return executeConsoleMcpToolDebug(body, csrfToken);
}

export function createSettingsMcpToolBinding(
  instanceId: string,
  body: SaveConsoleMcpToolBindingBody,
  csrfToken: string
) {
  return createConsoleMcpToolBinding(instanceId, body, csrfToken);
}

export function updateSettingsMcpToolBinding(
  bindingId: string,
  body: SaveConsoleMcpToolBindingBody,
  csrfToken: string
) {
  return updateConsoleMcpToolBinding(bindingId, body, csrfToken);
}

export function deleteSettingsMcpToolBinding(
  bindingId: string,
  csrfToken: string
) {
  return deleteConsoleMcpToolBinding(bindingId, csrfToken);
}

export function updateSettingsMcpInstanceDiscoveryPolicy(
  instanceId: string,
  body: UpdateConsoleMcpInstanceDiscoveryPolicyBody,
  csrfToken: string
) {
  return updateConsoleMcpInstanceDiscoveryPolicy(instanceId, body, csrfToken);
}
