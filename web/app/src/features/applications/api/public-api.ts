import {
  createConsoleApplicationApiKey,
  fetchConsoleApplicationApiDocsCatalog,
  fetchConsoleApplicationApiDocsCategoryOperations,
  fetchConsoleApplicationApiOperationSpec,
  getConsoleWorkflowScheduleTrigger,
  getConsoleApplicationApiMapping,
  getConsoleApplicationApiPublication,
  listConsoleApplicationApiKeys,
  publishConsoleApplicationApiVersion,
  replaceConsoleApplicationApiMapping,
  replaceConsoleWorkflowScheduleTrigger,
  revokeConsoleApplicationApiKey,
  unpublishConsoleApplicationApiVersion,
  updateConsoleApplicationApiStatus,
  type ConsoleApplicationApiKey,
  type ConsoleApplicationApiMapping,
  type ConsoleApplicationApiPublication,
  type ConsoleApiDocsCategoryOperationsRequest,
  type ConsoleWorkflowScheduleTrigger,
  type CreatedConsoleApplicationApiKey,
  type ReplaceConsoleWorkflowScheduleTriggerInput
} from '@1flowbase/api-client';

import { getApplicationsApiBaseUrl } from './applications';

export type ApplicationApiKey = ConsoleApplicationApiKey;
export type CreatedApplicationApiKey = CreatedConsoleApplicationApiKey;
export type ApplicationApiMapping = ConsoleApplicationApiMapping;
export type ApplicationApiPublication = ConsoleApplicationApiPublication;
export type WorkflowScheduleTrigger = ConsoleWorkflowScheduleTrigger;
export type WorkflowScheduleTriggerInput =
  ReplaceConsoleWorkflowScheduleTriggerInput;
export type ApplicationApiDocsCategoryOperationsRequest =
  ConsoleApiDocsCategoryOperationsRequest;

function normalizeApplicationApiDocsLocale(locale: string | null | undefined) {
  if (!locale) {
    return null;
  }

  const normalized = locale.replace('_', '-').toLowerCase();
  if (normalized.startsWith('zh')) {
    return 'zh_Hans';
  }
  if (normalized.startsWith('en')) {
    return 'en_US';
  }

  return null;
}

export function getApplicationApiDocsLocale() {
  if (typeof navigator === 'undefined') {
    return null;
  }

  const languages = navigator.languages?.length
    ? navigator.languages
    : [navigator.language];

  for (const language of languages) {
    const locale = normalizeApplicationApiDocsLocale(language);
    if (locale) {
      return locale;
    }
  }

  return null;
}

export const applicationApiKeysQueryKey = (applicationId: string) =>
  ['applications', applicationId, 'public-api', 'keys'] as const;
export const applicationApiMappingQueryKey = (applicationId: string) =>
  ['applications', applicationId, 'public-api', 'mapping'] as const;
export const applicationApiPublicationQueryKey = (applicationId: string) =>
  ['applications', applicationId, 'public-api', 'publication'] as const;
export const workflowScheduleTriggerQueryKey = (applicationId: string) =>
  ['applications', applicationId, 'workflow', 'schedule-trigger'] as const;
export const applicationApiDocsCatalogQueryKey = (
  applicationId: string,
  locale?: string | null
) =>
  [
    'applications',
    applicationId,
    'public-api',
    'docs',
    'catalog',
    locale ?? 'default'
  ] as const;
export const applicationApiDocsCategoryOperationsQueryKey = (
  applicationId: string,
  categoryId: string,
  locale?: string | null
) =>
  [
    'applications',
    applicationId,
    'public-api',
    'docs',
    'category',
    categoryId,
    'operations',
    locale ?? 'default'
  ] as const;
export const applicationApiDocsOperationSpecQueryKey = (
  applicationId: string,
  operationId: string,
  locale?: string | null
) =>
  [
    'applications',
    applicationId,
    'public-api',
    'docs',
    'operation',
    operationId,
    'openapi',
    locale ?? 'default'
  ] as const;

export function fetchApplicationApiKeys(applicationId: string) {
  return listConsoleApplicationApiKeys(
    applicationId,
    getApplicationsApiBaseUrl()
  );
}

export function createApplicationApiKey(
  applicationId: string,
  name: string,
  csrfToken: string
) {
  return createConsoleApplicationApiKey(
    applicationId,
    { name, expires_at: null },
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function revokeApplicationApiKey(
  applicationId: string,
  keyId: string,
  csrfToken: string
) {
  return revokeConsoleApplicationApiKey(
    applicationId,
    keyId,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function fetchApplicationApiMapping(applicationId: string) {
  return getConsoleApplicationApiMapping(
    applicationId,
    getApplicationsApiBaseUrl()
  );
}

export function saveApplicationApiMapping(
  applicationId: string,
  mapping: ApplicationApiMapping,
  csrfToken: string
) {
  return replaceConsoleApplicationApiMapping(
    applicationId,
    mapping,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function fetchApplicationApiPublication(applicationId: string) {
  return getConsoleApplicationApiPublication(
    applicationId,
    getApplicationsApiBaseUrl()
  );
}

export function publishApplicationApiVersion(
  applicationId: string,
  mapping: ApplicationApiMapping,
  csrfToken: string
) {
  return publishConsoleApplicationApiVersion(
    applicationId,
    { mapping, api_enabled: true },
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function unpublishApplicationApiVersion(
  applicationId: string,
  csrfToken: string
) {
  return unpublishConsoleApplicationApiVersion(
    applicationId,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function setApplicationApiEnabled(
  applicationId: string,
  apiEnabled: boolean,
  csrfToken: string
) {
  return updateConsoleApplicationApiStatus(
    applicationId,
    { api_enabled: apiEnabled },
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function fetchWorkflowScheduleTrigger(applicationId: string) {
  return getConsoleWorkflowScheduleTrigger(
    applicationId,
    getApplicationsApiBaseUrl()
  );
}

export function saveWorkflowScheduleTrigger(
  applicationId: string,
  input: WorkflowScheduleTriggerInput,
  csrfToken: string
) {
  return replaceConsoleWorkflowScheduleTrigger(
    applicationId,
    input,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function fetchApplicationApiDocsCatalog(
  applicationId: string,
  locale = getApplicationApiDocsLocale()
) {
  return fetchConsoleApplicationApiDocsCatalog(
    applicationId,
    getApplicationsApiBaseUrl(),
    locale
  );
}

export function fetchApplicationApiDocsCategoryOperations(
  applicationId: string,
  categoryId: string,
  request: ApplicationApiDocsCategoryOperationsRequest = {},
  locale = getApplicationApiDocsLocale()
) {
  return fetchConsoleApplicationApiDocsCategoryOperations(
    applicationId,
    categoryId,
    request,
    getApplicationsApiBaseUrl(),
    locale
  );
}

export function fetchApplicationApiDocsOperationSpec(
  applicationId: string,
  operationId: string,
  locale = getApplicationApiDocsLocale()
) {
  return fetchConsoleApplicationApiOperationSpec(
    applicationId,
    operationId,
    getApplicationsApiBaseUrl(),
    locale
  );
}
