import {
  createConsoleApplication,
  createConsoleApplicationTag,
  deleteConsoleApplication,
  exportConsoleAgentFlowTemplate,
  getConsoleApplication,
  getConsoleApplicationCatalog,
  getDefaultApiBaseUrl,
  importConsoleAgentFlowTemplate,
  listConsoleApplicationEnvironmentVariables,
  listConsoleApplications,
  previewConsoleAgentFlowTemplate,
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

export function exportAgentFlowTemplate(applicationId: string) {
  return exportConsoleAgentFlowTemplate(
    applicationId,
    getApplicationsApiBaseUrl()
  );
}

export function previewAgentFlowTemplate(template: AgentFlowTemplatePackage) {
  return previewConsoleAgentFlowTemplate(
    { template },
    getApplicationsApiBaseUrl()
  );
}

export function importAgentFlowTemplate(
  input: ImportAgentFlowTemplateInput,
  csrfToken: string
) {
  return importConsoleAgentFlowTemplate(
    input,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}
