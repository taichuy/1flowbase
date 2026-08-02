import {
  deleteConsoleOfficialAgentFlowTemplateRelease,
  getDefaultApiBaseUrl,
  importConsoleOfficialAgentFlowTemplate,
  listConsoleOfficialAgentFlowTemplateCatalog,
  previewConsoleOfficialAgentFlowTemplate,
  repairConsoleOfficialAgentFlowTemplateRelease,
  switchConsoleOfficialAgentFlowTemplateCurrent,
  syncConsoleOfficialAgentFlowTemplate,
  type ApiBaseUrlLocation,
  type ConsoleAgentFlowTemplateLibraryEntry,
  type ConsoleAgentFlowTemplateLocalVersion,
  type ConsoleAgentFlowTemplatePreview,
  type ConsoleOfficialAgentFlowTemplateCatalog,
  type ImportConsoleAgentFlowTemplateResponse
} from '@1flowbase/api-client';

export type OfficialAgentFlowTemplateCatalog =
  ConsoleOfficialAgentFlowTemplateCatalog;
export type AgentFlowTemplateLibraryEntry =
  ConsoleAgentFlowTemplateLibraryEntry;
export type AgentFlowTemplateLocalVersion =
  ConsoleAgentFlowTemplateLocalVersion;
export type AgentFlowTemplatePreview = ConsoleAgentFlowTemplatePreview;
export type ImportAgentFlowTemplateResponse =
  ImportConsoleAgentFlowTemplateResponse;

export const officialAgentFlowTemplateCatalogQueryKey = [
  'templates',
  'official-agent-flow',
  'catalog'
] as const;

export function getTemplatesApiBaseUrl(
  locationLike: ApiBaseUrlLocation | undefined = typeof window !== 'undefined'
    ? window.location
    : undefined
): string {
  return (
    import.meta.env.VITE_API_BASE_URL ?? getDefaultApiBaseUrl(locationLike)
  );
}

export function fetchOfficialAgentFlowTemplateCatalog() {
  return listConsoleOfficialAgentFlowTemplateCatalog(getTemplatesApiBaseUrl());
}

export function previewOfficialAgentFlowTemplate(
  templateId: string,
  releaseVersion: number | undefined,
  csrfToken: string
) {
  return previewConsoleOfficialAgentFlowTemplate(
    templateId,
    releaseVersion === undefined ? {} : { release_version: releaseVersion },
    csrfToken,
    getTemplatesApiBaseUrl()
  );
}

export function importOfficialAgentFlowTemplate(
  templateId: string,
  input: {
    release_version?: number;
    name?: string;
    description?: string;
  },
  csrfToken: string
) {
  return importConsoleOfficialAgentFlowTemplate(
    templateId,
    input,
    csrfToken,
    getTemplatesApiBaseUrl()
  );
}

export function syncOfficialAgentFlowTemplate(
  templateId: string,
  releaseVersion: number | undefined,
  csrfToken: string
) {
  return syncConsoleOfficialAgentFlowTemplate(
    templateId,
    releaseVersion === undefined ? {} : { release_version: releaseVersion },
    csrfToken,
    getTemplatesApiBaseUrl()
  );
}

export function switchOfficialAgentFlowTemplateCurrent(
  templateId: string,
  releaseVersion: number,
  csrfToken: string
) {
  return switchConsoleOfficialAgentFlowTemplateCurrent(
    templateId,
    releaseVersion,
    csrfToken,
    getTemplatesApiBaseUrl()
  );
}

export function deleteOfficialAgentFlowTemplateRelease(
  templateId: string,
  releaseVersion: number,
  csrfToken: string
) {
  return deleteConsoleOfficialAgentFlowTemplateRelease(
    templateId,
    releaseVersion,
    csrfToken,
    getTemplatesApiBaseUrl()
  );
}

export function repairOfficialAgentFlowTemplateRelease(
  templateId: string,
  releaseVersion: number,
  csrfToken: string
) {
  return repairConsoleOfficialAgentFlowTemplateRelease(
    templateId,
    releaseVersion,
    csrfToken,
    getTemplatesApiBaseUrl()
  );
}
