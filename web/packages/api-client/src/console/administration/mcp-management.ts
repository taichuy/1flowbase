import {
  type ConsoleMcpInstance,
  type ConsoleMcpClientCredential,
  type ConsoleMcpGroup,
  type ConsoleMcpTool,
  type ConsoleMcpBundlePreview,
  type ConsoleMcpBundleImportReport,
  type ExportConsoleMcpBundleBody,
  type ConsoleMcpBundleExportDefaults,
  type ConsoleOfficialMcpBundleCatalog,
  type ConsoleOfficialMcpBundleBody,
  type ConsoleMcpTemplateLibraryBundle,
  type ConsoleMcpTemplateLibrary,
  type ConsoleMcpTemplateLibraryVersionBody,
  type ConsoleMcpToolBinding,
  type ConsoleMcpInstanceDiscoveryPolicy,
  type ConsoleMcpCatalog,
  type ConsoleMcpInterfaceCapability,
  type ConsoleMcpListItemSummary,
  type ConsoleMcpExportPackage,
  type SaveConsoleMcpInstanceBody,
  type CopyConsoleMcpInstanceBody,
  type SaveConsoleMcpGroupBody,
  type MoveConsoleMcpGroupBody,
  type SaveConsoleMcpToolBody,
  type UpdateConsoleMcpToolBody,
  type SaveConsoleMcpToolBindingBody,
  type UpdateConsoleMcpInstanceDiscoveryPolicyBody,
  type ExecuteConsoleMcpToolDebugBody,
  type ConsoleMcpToolDebugExecuteResponse,
  type ConsoleMcpUpstreamConnection,
  type SaveConsoleMcpUpstreamConnectionBody,
  type SaveConsoleMcpUpstreamConnectionCredentialsBody,
  type ConsoleMcpUpstreamConnectionTestResult,
  type ConsoleMcpUpstreamConnectionDraftTestResult,
  type TestConsoleMcpUpstreamConnectionDraftBody,
  type ConsoleMcpUpstreamDiscoveryResult,
  type ImportConsoleMcpUpstreamToolsBody,
  type ExecuteConsoleMcpProxyToolDebugBody,
  type ConsoleMcpProxyToolDebugResponse
} from './mcp-management-types';
import { apiFetch, apiFetchBlob, apiFetchVoid } from '../../transport';

export function fetchConsoleMcpCatalog(baseUrl?: string) {
  return apiFetch<ConsoleMcpCatalog>({
    path: '/api/console/mcp/catalog',
    baseUrl
  });
}

export function fetchConsoleMcpInterfaceCapabilities(
  options: { bindable_only?: boolean } = {},
  baseUrl?: string
) {
  const params = new URLSearchParams();
  if (options.bindable_only !== undefined) {
    params.set('bindable_only', String(options.bindable_only));
  }
  const query = params.toString();
  return apiFetch<ConsoleMcpInterfaceCapability[]>({
    path: `/api/console/mcp/interface-capabilities${query ? `?${query}` : ''}`,
    baseUrl
  });
}

export function fetchConsoleMcpListItems(
  options: {
    instance_id?: string;
    path?: string;
    path_regex?: string;
    limit?: number;
  } = {},
  baseUrl?: string
) {
  const params = new URLSearchParams();
  if (options.instance_id) {
    params.set('instance_id', options.instance_id);
  }
  if (options.path) {
    params.set('path', options.path);
  }
  if (options.path_regex) {
    params.set('path_regex', options.path_regex);
  }
  if (options.limit !== undefined) {
    params.set('limit', String(options.limit));
  }
  const query = params.toString();
  return apiFetch<ConsoleMcpListItemSummary[]>({
    path: `/api/console/mcp/list${query ? `?${query}` : ''}`,
    baseUrl
  });
}

export function exportConsoleMcpCatalog(baseUrl?: string) {
  return apiFetch<ConsoleMcpExportPackage>({
    path: '/api/console/mcp/export',
    baseUrl
  });
}

export function fetchConsoleMcpBundleExportDefaults(baseUrl?: string) {
  return apiFetch<ConsoleMcpBundleExportDefaults>({
    path: '/api/console/mcp/bundles/export-defaults',
    baseUrl
  });
}

export function exportConsoleMcpBundle(
  body: ExportConsoleMcpBundleBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchBlob({
    path: '/api/console/mcp/bundles/export',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function exportConsoleMcpInstanceBundle(
  instanceId: string,
  body: ExportConsoleMcpBundleBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchBlob({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/bundles/export`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

function uploadConsoleMcpBundle<T>(
  path: string,
  file: File,
  csrfToken: string,
  baseUrl?: string
) {
  const formData = new FormData();
  formData.set('file', file);
  return apiFetch<T>({
    path,
    method: 'POST',
    rawBody: formData,
    contentType: null,
    csrfToken,
    baseUrl
  });
}

export function previewConsoleMcpBundle(
  file: File,
  csrfToken: string,
  baseUrl?: string
) {
  return uploadConsoleMcpBundle<ConsoleMcpBundlePreview>(
    '/api/console/mcp/bundles/preview-upload',
    file,
    csrfToken,
    baseUrl
  );
}

export function importConsoleMcpBundle(
  file: File,
  csrfToken: string,
  baseUrl?: string
) {
  return uploadConsoleMcpBundle<ConsoleMcpBundleImportReport>(
    '/api/console/mcp/bundles/import-upload',
    file,
    csrfToken,
    baseUrl
  );
}

export function fetchConsoleOfficialMcpBundles(baseUrl?: string) {
  return apiFetch<ConsoleOfficialMcpBundleCatalog>({
    path: '/api/console/mcp/bundles/official',
    baseUrl
  });
}

export function previewConsoleOfficialMcpBundle(
  body: ConsoleOfficialMcpBundleBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpBundlePreview>({
    path: '/api/console/mcp/bundles/preview-official',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function importConsoleOfficialMcpBundle(
  body: ConsoleOfficialMcpBundleBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpBundleImportReport>({
    path: '/api/console/mcp/bundles/import-official',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleMcpTemplateLibrary(baseUrl?: string) {
  return apiFetch<ConsoleMcpTemplateLibrary>({
    path: '/api/console/mcp/bundles/library',
    baseUrl
  });
}

export function refreshConsoleMcpTemplateLibrary(baseUrl?: string) {
  return apiFetch<ConsoleMcpTemplateLibrary>({
    path: '/api/console/mcp/bundles/library?refresh_remote=true',
    baseUrl
  });
}

function mcpTemplateLibraryPath(
  organization: string,
  bundleId: string,
  suffix: string
) {
  return `/api/console/mcp/bundles/library/${encodeURIComponent(organization)}/${encodeURIComponent(bundleId)}/${suffix}`;
}

export function syncConsoleMcpTemplateLibraryBundle(
  organization: string,
  bundleId: string,
  body: ConsoleMcpTemplateLibraryVersionBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpTemplateLibraryBundle>({
    path: mcpTemplateLibraryPath(organization, bundleId, 'sync'),
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function previewConsoleMcpTemplateLibraryBundle(
  organization: string,
  bundleId: string,
  body: ConsoleMcpTemplateLibraryVersionBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpBundlePreview>({
    path: mcpTemplateLibraryPath(organization, bundleId, 'preview'),
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function importConsoleMcpTemplateLibraryBundle(
  organization: string,
  bundleId: string,
  body: ConsoleMcpTemplateLibraryVersionBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpBundleImportReport>({
    path: mcpTemplateLibraryPath(organization, bundleId, 'import'),
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function setConsoleMcpTemplateLibraryCurrentVersion(
  organization: string,
  bundleId: string,
  bundleVersion: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: mcpTemplateLibraryPath(
      organization,
      bundleId,
      `current/${encodeURIComponent(bundleVersion)}`
    ),
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpTemplateLibraryRelease(
  organization: string,
  bundleId: string,
  bundleVersion: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: mcpTemplateLibraryPath(
      organization,
      bundleId,
      `releases/${encodeURIComponent(bundleVersion)}`
    ),
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function repairConsoleMcpTemplateLibraryRelease(
  organization: string,
  bundleId: string,
  bundleVersion: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: mcpTemplateLibraryPath(
      organization,
      bundleId,
      `releases/${encodeURIComponent(bundleVersion)}/repair`
    ),
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleMcpTool(toolId: string, baseUrl?: string) {
  return apiFetch<ConsoleMcpTool>({
    path: `/api/console/mcp/tools/${encodeURIComponent(toolId)}`,
    baseUrl
  });
}

export function createConsoleMcpInstance(
  body: SaveConsoleMcpInstanceBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpInstance>({
    path: '/api/console/mcp/instances',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleMcpInstance(
  instanceId: string,
  body: SaveConsoleMcpInstanceBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpInstance>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}`,
    method: 'PUT',
    body,
    csrfToken,
    baseUrl
  });
}

export function copyConsoleMcpInstance(
  sourceInstanceId: string,
  body: CopyConsoleMcpInstanceBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpInstance>({
    path: `/api/console/mcp/instances/${encodeURIComponent(sourceInstanceId)}/copy`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpInstance(
  instanceId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleMcpClientCredential(
  instanceId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpClientCredential>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/client-credential`,
    baseUrl
  });
}

export function saveConsoleMcpClientCredential(
  instanceId: string,
  apiKey: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpClientCredential>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/client-credential`,
    method: 'PUT',
    body: { api_key: apiKey },
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpClientCredential(
  instanceId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/client-credential`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function upsertConsoleMcpGroup(
  instanceId: string,
  body: SaveConsoleMcpGroupBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpGroup>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/groups`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function moveConsoleMcpGroup(
  instanceId: string,
  body: MoveConsoleMcpGroupBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpGroup>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/groups/move`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpGroup(
  instanceId: string,
  path: string,
  csrfToken: string,
  baseUrl?: string
) {
  const params = new URLSearchParams({ path });
  return apiFetchVoid({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/groups?${params.toString()}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function createConsoleMcpTool(
  body: SaveConsoleMcpToolBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpTool>({
    path: '/api/console/mcp/tools',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleMcpTool(
  toolId: string,
  body: UpdateConsoleMcpToolBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpTool>({
    path: `/api/console/mcp/tools/${encodeURIComponent(toolId)}`,
    method: 'PUT',
    body,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpTool(
  toolId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/mcp/tools/${encodeURIComponent(toolId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function refreshConsoleMcpToolDescription(
  toolId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpTool>({
    path: `/api/console/mcp/tools/${encodeURIComponent(toolId)}/description/refresh`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function executeConsoleMcpToolDebug(
  body: ExecuteConsoleMcpToolDebugBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpToolDebugExecuteResponse>({
    path: '/api/console/mcp/debug/execute',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function createConsoleMcpToolBinding(
  instanceId: string,
  body: SaveConsoleMcpToolBindingBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpToolBinding>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/tool-bindings`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleMcpToolBinding(
  bindingId: string,
  body: SaveConsoleMcpToolBindingBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpToolBinding>({
    path: `/api/console/mcp/tool-bindings/${encodeURIComponent(bindingId)}`,
    method: 'PUT',
    body,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpToolBinding(
  bindingId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/mcp/tool-bindings/${encodeURIComponent(bindingId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function updateConsoleMcpInstanceDiscoveryPolicy(
  instanceId: string,
  body: UpdateConsoleMcpInstanceDiscoveryPolicyBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpInstanceDiscoveryPolicy>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/discovery-policy`,
    method: 'PUT',
    body,
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleMcpInstanceDiscoveryPolicy(
  instanceId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpInstanceDiscoveryPolicy>({
    path: `/api/console/mcp/instances/${encodeURIComponent(instanceId)}/discovery-policy`,
    baseUrl
  });
}

export function fetchConsoleMcpUpstreamConnections(baseUrl?: string) {
  return apiFetch<ConsoleMcpUpstreamConnection[]>({
    path: '/api/console/mcp/upstream-connections',
    baseUrl
  });
}

export function createConsoleMcpUpstreamConnection(
  body: SaveConsoleMcpUpstreamConnectionBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpUpstreamConnection>({
    path: '/api/console/mcp/upstream-connections',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleMcpUpstreamConnection(
  connectionId: string,
  body: SaveConsoleMcpUpstreamConnectionBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpUpstreamConnection>({
    path: `/api/console/mcp/upstream-connections/${encodeURIComponent(connectionId)}`,
    method: 'PUT',
    body,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpUpstreamConnection(
  connectionId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/mcp/upstream-connections/${encodeURIComponent(connectionId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function saveConsoleMcpUpstreamConnectionCredentials(
  connectionId: string,
  body: SaveConsoleMcpUpstreamConnectionCredentialsBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/mcp/upstream-connections/${encodeURIComponent(connectionId)}/credentials`,
    method: 'PUT',
    body,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleMcpUpstreamConnectionCredentials(
  connectionId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/mcp/upstream-connections/${encodeURIComponent(connectionId)}/credentials`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function testConsoleMcpUpstreamConnection(
  connectionId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpUpstreamConnectionTestResult>({
    path: `/api/console/mcp/upstream-connections/${encodeURIComponent(connectionId)}/test`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function testConsoleMcpUpstreamConnectionDraft(
  body: TestConsoleMcpUpstreamConnectionDraftBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpUpstreamConnectionDraftTestResult>({
    path: '/api/console/mcp/upstream-connections/test',
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function discoverConsoleMcpUpstreamConnection(
  connectionId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpUpstreamDiscoveryResult>({
    path: `/api/console/mcp/upstream-connections/${encodeURIComponent(connectionId)}/discover`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function importConsoleMcpUpstreamTools(
  connectionId: string,
  body: ImportConsoleMcpUpstreamToolsBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpTool[]>({
    path: `/api/console/mcp/upstream-connections/${encodeURIComponent(connectionId)}/imports`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export function executeConsoleMcpProxyToolDebug(
  toolId: string,
  body: ExecuteConsoleMcpProxyToolDebugBody,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleMcpProxyToolDebugResponse>({
    path: `/api/console/mcp/tools/${encodeURIComponent(toolId)}/debug`,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
}

export {
  type ConsoleMcpInstance,
  type WebMcpExposure,
  type ConsoleMcpManagedBundleSource,
  type ConsoleMcpLlmToolRegistration,
  type ConsoleMcpClientCredential,
  type ConsoleMcpGroup,
  type ConsoleMcpInterfaceWrapperExecutionTarget,
  type ConsoleMcpProxyExecutionTarget,
  type ConsoleMcpAssistantClientExecutionTarget,
  type ConsoleMcpToolExecutionTarget,
  type ConsoleMcpToolAvailabilityStatus,
  type ConsoleMcpProxyInputMappingEntry,
  type ConsoleMcpProxyInputMapping,
  type ConsoleMcpProxyOutputMappingEntry,
  type ConsoleMcpProxyOutputMapping,
  type ConsoleMcpTool,
  type ConsoleMcpBundleManifest,
  type ConsoleMcpBundleVersionStatus,
  type ConsoleMcpBundleItemEffect,
  type ConsoleMcpBundleEffectSummary,
  type ConsoleMcpBundleItemReport,
  type ConsoleMcpBundlePreview,
  type ConsoleMcpBundleImportReport,
  type ExportConsoleMcpBundleBody,
  type ConsoleMcpBundleExportDefaults,
  type ConsoleOfficialMcpBundleEntry,
  type ConsoleOfficialMcpBundleCatalog,
  type ConsoleOfficialMcpBundleBody,
  type ConsoleMcpTemplateLibraryVersion,
  type ConsoleMcpTemplateLibraryBundle,
  type ConsoleMcpTemplateLibrary,
  type ConsoleMcpTemplateLibraryVersionBody,
  type ConsoleMcpToolBinding,
  type ConsoleMcpInstanceDiscoveryPolicy,
  type ConsoleMcpParameterType,
  type ConsoleMcpParameterDescriptor,
  type ConsoleMcpCatalog,
  type ConsoleMcpInterfaceCapability,
  type ConsoleMcpListItemSummary,
  type ConsoleMcpExportPackage,
  type SaveConsoleMcpInstanceBody,
  type CopyConsoleMcpInstanceBody,
  type SaveConsoleMcpGroupBody,
  type MoveConsoleMcpGroupBody,
  type SaveConsoleMcpToolBody,
  type UpdateConsoleMcpToolBody,
  type SaveConsoleMcpToolBindingBody,
  type UpdateConsoleMcpInstanceDiscoveryPolicyBody,
  type ConsoleMcpToolDebugResponseMode,
  type ExecuteConsoleMcpToolDebugBody,
  type ConsoleMcpToolDebugExecuteResponse,
  type ConsoleMcpToolDebugDetailsResponse,
  type ConsoleMcpUpstreamTransport,
  type ConsoleMcpUpstreamAuthType,
  type ConsoleMcpUpstreamConnectionStatus,
  type ConsoleMcpUpstreamCredentialsStatus,
  type ConsoleMcpUpstreamConnection,
  type SaveConsoleMcpUpstreamConnectionBody,
  type SaveConsoleMcpUpstreamConnectionCredentialsBody,
  type ConsoleMcpUpstreamConnectionTestResult,
  type ConsoleMcpUpstreamConnectionDraftTestResult,
  type TestConsoleMcpUpstreamConnectionDraftBody,
  type ConsoleMcpUpstreamToolSourceStatus,
  type ConsoleMcpUpstreamDiscoveredTool,
  type ConsoleMcpUpstreamDiscoveryResult,
  type ImportConsoleMcpUpstreamToolsBody,
  type ExecuteConsoleMcpProxyToolDebugBody,
  type ConsoleMcpProxyToolDebugResponse
} from './mcp-management-types';
