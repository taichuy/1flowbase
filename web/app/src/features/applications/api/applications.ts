import {
  createConsoleApplication,
  createConsoleApplicationTag,
  deleteConsoleApplication,
  exportConsoleApplicationArchive,
  getConsoleApplication,
  getConsoleApplicationCatalog,
  getDefaultApiBaseUrl,
  importConsoleApplicationArchive,
  importConsoleInstalledApplicationExtension,
  listConsoleApplicationEnvironmentVariables,
  listConsoleApplications,
  listConsoleInstalledExtensions,
  previewConsoleApplicationArchive,
  previewConsoleInstalledApplicationExtension,
  replaceConsoleApplicationEnvironmentVariables,
  updateConsoleApplication,
  type ApiBaseUrlLocation,
  type ConsoleAgentFlowTemplatePackage,
  type ConsoleAgentFlowTemplatePreview,
  type ImportConsoleAgentFlowTemplateInput,
  type ImportConsoleAgentFlowTemplateResponse,
  type ConsoleApplicationCatalog,
  type ConsoleApplicationDetail,
  type ConsoleApplicationEnvironmentVariable,
  type ConsoleApplicationSummary,
  type ConsoleApplicationTagCatalogEntry,
  type ConsoleInstalledExtension,
  type CreateConsoleApplicationInput,
  type UpdateConsoleApplicationInput
} from '@1flowbase/api-client';

export type Application = ConsoleApplicationSummary;
export type ApplicationDetail = ConsoleApplicationDetail;
export type ApplicationCatalog = ConsoleApplicationCatalog;
export type ApplicationEnvironmentVariable =
  ConsoleApplicationEnvironmentVariable;
export interface ApplicationEnvironmentVariableInput {
  name: string;
  value_type: string;
  value: unknown;
  description: string;
}
export type ApplicationTagCatalogEntry = ConsoleApplicationTagCatalogEntry;
export type CreateApplicationInput = CreateConsoleApplicationInput;
export type AgentFlowTemplatePackage = ConsoleAgentFlowTemplatePackage;
export type AgentFlowTemplatePreview = ConsoleAgentFlowTemplatePreview;
export type ImportAgentFlowTemplateInput = ImportConsoleAgentFlowTemplateInput;
export type ImportAgentFlowTemplateResponse =
  ImportConsoleAgentFlowTemplateResponse;
export type UpdateApplicationInput = UpdateConsoleApplicationInput;
export type InstalledAgentFlow = ConsoleInstalledExtension;
export interface CreateApplicationTagInput {
  name: string;
}

export const applicationsQueryKey = ['applications'] as const;
export const applicationCatalogQueryKey = ['applications', 'catalog'] as const;
export const applicationDetailQueryKey = (applicationId: string) =>
  ['applications', applicationId] as const;
export const applicationEnvironmentVariablesQueryKey = (
  applicationId: string
) => ['applications', applicationId, 'environment-variables'] as const;
export const installedAgentFlowsQueryKey = [
  'applications',
  'installed-agent-flows'
] as const;

export function fetchInstalledAgentFlows() {
  return listConsoleInstalledExtensions(undefined, 50, 'agent-flow');
}

export function getApplicationsApiBaseUrl(
  locationLike: ApiBaseUrlLocation | undefined = typeof window !== 'undefined'
    ? window.location
    : undefined
): string {
  return (
    import.meta.env.VITE_API_BASE_URL ?? getDefaultApiBaseUrl(locationLike)
  );
}

export function fetchApplications(): Promise<Application[]> {
  return listConsoleApplications(getApplicationsApiBaseUrl());
}

export function fetchApplicationCatalog(): Promise<ApplicationCatalog> {
  return getConsoleApplicationCatalog(getApplicationsApiBaseUrl());
}

export function fetchApplicationDetail(
  applicationId: string
): Promise<ApplicationDetail> {
  return getConsoleApplication(applicationId, getApplicationsApiBaseUrl());
}

export function createApplication(
  input: CreateApplicationInput,
  csrfToken: string
) {
  return createConsoleApplication(
    input,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function updateApplication(
  applicationId: string,
  input: UpdateApplicationInput,
  csrfToken: string
) {
  return updateConsoleApplication(
    applicationId,
    input,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function deleteApplication(applicationId: string, csrfToken: string) {
  return deleteConsoleApplication(
    applicationId,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function fetchApplicationEnvironmentVariables(
  applicationId: string
): Promise<ApplicationEnvironmentVariable[]> {
  return listConsoleApplicationEnvironmentVariables(
    applicationId,
    getApplicationsApiBaseUrl()
  );
}

export function replaceApplicationEnvironmentVariables(
  applicationId: string,
  variables: ApplicationEnvironmentVariableInput[],
  csrfToken: string
) {
  return replaceConsoleApplicationEnvironmentVariables(
    applicationId,
    {
      variables: variables.map(({ name, value_type, value, description }) => ({
        name,
        value_type,
        value,
        description
      }))
    },
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function createApplicationTag(
  input: CreateApplicationTagInput,
  csrfToken: string
) {
  return createConsoleApplicationTag(
    input,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function exportApplicationArchive(applicationIds: string[]) {
  return exportConsoleApplicationArchive(
    { application_ids: applicationIds },
    getApplicationsApiBaseUrl()
  );
}

export function previewAgentFlowTemplate(template: AgentFlowTemplatePackage) {
  return previewConsoleApplicationArchive(
    new Blob([JSON.stringify(template)], { type: 'application/json' }),
    'official-template.json',
    getApplicationsApiBaseUrl()
  );
}

export function importAgentFlowTemplate(
  input: ImportAgentFlowTemplateInput,
  csrfToken: string
) {
  return importConsoleApplicationArchive(
    {
      file: new Blob([JSON.stringify(input.template)], {
        type: 'application/json'
      }),
      filename: 'official-template.json',
      name: input.name,
      description: input.description
    },
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function previewInstalledApplicationExtension(
  extensionInstallationId: string
) {
  return previewConsoleInstalledApplicationExtension(
    extensionInstallationId,
    getApplicationsApiBaseUrl()
  );
}

export function importInstalledApplicationExtension(
  extensionInstallationId: string,
  input: Parameters<typeof importConsoleInstalledApplicationExtension>[1],
  csrfToken: string
) {
  return importConsoleInstalledApplicationExtension(
    extensionInstallationId,
    input,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function previewApplicationArchive(file: File) {
  return previewConsoleApplicationArchive(
    file,
    file.name,
    getApplicationsApiBaseUrl()
  );
}

export function importApplicationArchive(
  file: File,
  input: { name?: string; description?: string },
  csrfToken: string
) {
  return importConsoleApplicationArchive(
    {
      file,
      filename: file.name,
      name: input.name,
      description: input.description
    },
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}
