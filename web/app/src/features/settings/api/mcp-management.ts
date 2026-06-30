import {
  createConsoleMcpInstance,
  createConsoleMcpTool,
  createConsoleMcpToolBinding,
  deleteConsoleMcpGroup,
  deleteConsoleMcpInstance,
  deleteConsoleMcpTool,
  deleteConsoleMcpToolBinding,
  executeConsoleMcpToolDebug,
  exportConsoleMcpCatalog,
  exportConsoleMcpInstanceDirectory,
  fetchConsoleMcpCatalog,
  fetchConsoleMcpInterfaceCapabilities,
  refreshConsoleMcpToolDescription,
  updateConsoleMcpInstance,
  updateConsoleMcpMetaToolConfig,
  updateConsoleMcpTool,
  updateConsoleMcpToolBinding,
  upsertConsoleMcpGroup,
  type ConsoleMcpCatalog,
  type ConsoleMcpInterfaceCapability,
  type ConsoleMcpToolDebugExecuteResponse,
  type ExecuteConsoleMcpToolDebugBody,
  type SaveConsoleMcpGroupBody,
  type SaveConsoleMcpInstanceBody,
  type SaveConsoleMcpToolBindingBody,
  type SaveConsoleMcpToolBody,
  type UpdateConsoleMcpMetaToolConfigBody,
  type UpdateConsoleMcpToolBody
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

export function fetchSettingsMcpCatalog() {
  return fetchConsoleMcpCatalog();
}

export function fetchSettingsMcpInterfaceCapabilities() {
  return fetchConsoleMcpInterfaceCapabilities({ bindable_only: false });
}

export function exportSettingsMcpCatalog() {
  return exportConsoleMcpCatalog();
}

export function exportSettingsMcpInstanceDirectory() {
  return exportConsoleMcpInstanceDirectory();
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

export function deleteSettingsMcpInstance(instanceId: string, csrfToken: string) {
  return deleteConsoleMcpInstance(instanceId, csrfToken);
}

export function upsertSettingsMcpGroup(
  instanceId: string,
  body: SaveConsoleMcpGroupBody,
  csrfToken: string
) {
  return upsertConsoleMcpGroup(instanceId, body, csrfToken);
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

export function deleteSettingsMcpToolBinding(bindingId: string, csrfToken: string) {
  return deleteConsoleMcpToolBinding(bindingId, csrfToken);
}

export function updateSettingsMcpMetaToolConfig(
  body: UpdateConsoleMcpMetaToolConfigBody,
  csrfToken: string
) {
  return updateConsoleMcpMetaToolConfig(body, csrfToken);
}
