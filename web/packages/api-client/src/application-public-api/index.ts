import type {
  ConsoleApiDocsCatalog,
  ConsoleApiDocsCategoryOperations,
  ConsoleApiDocsCategoryOperationsRequest
} from '../console/api-docs';
import { apiFetch, apiFetchVoid } from '../transport';

export const APPLICATION_PUBLIC_RUNTIME_PATHS = {
  nativeRuns: '/api/agent/v1/runs',
  nativeFiles: '/api/agent/v1/files',
  openAiChatCompletions: '/v1/chat/completions',
  anthropicMessages: '/v1/messages'
} as const;

export interface ConsoleApplicationApiKey {
  id: string;
  name: string;
  token_prefix: string;
  creator_user_id: string;
  enabled: boolean;
  expires_at: string | null;
  last_used_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreatedConsoleApplicationApiKey extends ConsoleApplicationApiKey {
  token: string;
}

export interface CreateConsoleApplicationApiKeyInput {
  name: string;
  expires_at: string | null;
}

export interface ConsoleApplicationApiMapping {
  input: {
    query_target: string;
    model_target: string | null;
    inputs_target: string | null;
    history_target: string | null;
    attachments_target: string | null;
  };
  output: {
    answer_selector: string | null;
    usage_selector: string | null;
    files_selector: string | null;
    error_selector: string | null;
  };
  extension?: ConsoleWorkflowExtensionApiConfig | null;
}

export type ConsoleWorkflowExtensionHttpMethod =
  | 'GET'
  | 'POST'
  | 'PUT'
  | 'PATCH'
  | 'DELETE'
  | 'HEAD'
  | 'OPTIONS';

export type ConsoleWorkflowExtensionResponseMode = 'sync' | 'async';

export type ConsoleWorkflowExtensionParameterSource =
  | 'path'
  | 'query'
  | 'form'
  | 'body';

export interface ConsoleWorkflowExtensionParameterMapping {
  name: string;
  source: ConsoleWorkflowExtensionParameterSource;
  target: string;
}

export interface ConsoleWorkflowExtensionApiConfig {
  slug: string;
  method: ConsoleWorkflowExtensionHttpMethod;
  response_mode: ConsoleWorkflowExtensionResponseMode;
  parameters: ConsoleWorkflowExtensionParameterMapping[];
}

export interface ConsoleApplicationApiPublication {
  id: string;
  application_id: string;
  flow_id: string;
  flow_version_id: string;
  compiled_plan_id: string;
  version_sequence: number;
  active: boolean;
  api_enabled: boolean;
  mapping_snapshot: ConsoleApplicationApiMapping;
  public_url: string;
  created_by: string;
  created_at: string;
}

export interface PublishConsoleApplicationApiVersionInput {
  mapping: ConsoleApplicationApiMapping;
  api_enabled: boolean;
}

export interface UpdateConsoleApplicationApiStatusInput {
  api_enabled: boolean;
}

export interface ConsoleApplicationApiStatus {
  application_id: string;
  api_enabled: boolean;
  public_url: string;
}

export interface ConsoleWorkflowScheduleTrigger {
  id: string;
  workspace_id: string;
  application_id: string;
  enabled: boolean;
  cron: string;
  timezone: string;
  input_payload: unknown;
  created_by: string;
  updated_by: string;
  created_at: string;
  updated_at: string;
}

export interface ReplaceConsoleWorkflowScheduleTriggerInput {
  enabled: boolean;
  cron: string;
  timezone: string;
  input_payload: unknown;
}

function buildApplicationApiDocsPath(path: string, locale?: string | null) {
  if (!locale) {
    return path;
  }

  const params = new URLSearchParams();
  params.set('locale', locale);
  return `${path}?${params.toString()}`;
}

function buildApplicationApiDocsCategoryOperationsPath(
  path: string,
  request: ConsoleApiDocsCategoryOperationsRequest = {},
  locale?: string | null
) {
  const params = new URLSearchParams();

  if (locale) {
    params.set('locale', locale);
  }

  if (request.offset !== undefined) {
    params.set('offset', String(request.offset));
  }

  if (request.limit !== undefined) {
    params.set('limit', String(request.limit));
  }

  if (request.q) {
    params.set('q', request.q);
  }

  const query = params.size > 0 ? `?${params.toString()}` : '';
  return `${path}${query}`;
}

export function listConsoleApplicationApiKeys(
  applicationId: string,
  baseUrl?: string
): Promise<ConsoleApplicationApiKey[]> {
  return apiFetch<ConsoleApplicationApiKey[]>({
    path: `/api/console/applications/${applicationId}/api-keys`,
    baseUrl
  });
}

export function createConsoleApplicationApiKey(
  applicationId: string,
  input: CreateConsoleApplicationApiKeyInput,
  csrfToken: string,
  baseUrl?: string
): Promise<CreatedConsoleApplicationApiKey> {
  return apiFetch<CreatedConsoleApplicationApiKey>({
    path: `/api/console/applications/${applicationId}/api-keys`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function revokeConsoleApplicationApiKey(
  applicationId: string,
  keyId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: `/api/console/applications/${applicationId}/api-keys/${keyId}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function getConsoleApplicationApiMapping(
  applicationId: string,
  baseUrl?: string
): Promise<ConsoleApplicationApiMapping> {
  return apiFetch<ConsoleApplicationApiMapping>({
    path: `/api/console/applications/${applicationId}/api-mapping`,
    baseUrl
  });
}

export function replaceConsoleApplicationApiMapping(
  applicationId: string,
  mapping: ConsoleApplicationApiMapping,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleApplicationApiMapping> {
  return apiFetch<ConsoleApplicationApiMapping>({
    path: `/api/console/applications/${applicationId}/api-mapping`,
    method: 'PUT',
    body: mapping,
    csrfToken,
    baseUrl
  });
}

export function getConsoleApplicationApiPublication(
  applicationId: string,
  baseUrl?: string
): Promise<ConsoleApplicationApiPublication> {
  return apiFetch<ConsoleApplicationApiPublication>({
    path: `/api/console/applications/${applicationId}/api-publication`,
    baseUrl
  });
}

export function publishConsoleApplicationApiVersion(
  applicationId: string,
  input: PublishConsoleApplicationApiVersionInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleApplicationApiPublication> {
  return apiFetch<ConsoleApplicationApiPublication>({
    path: `/api/console/applications/${applicationId}/api-publications`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleApplicationApiStatus(
  applicationId: string,
  input: UpdateConsoleApplicationApiStatusInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleApplicationApiStatus> {
  return apiFetch<ConsoleApplicationApiStatus>({
    path: `/api/console/applications/${applicationId}/api-status`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function getConsoleWorkflowScheduleTrigger(
  applicationId: string,
  baseUrl?: string
): Promise<ConsoleWorkflowScheduleTrigger | null> {
  return apiFetch<ConsoleWorkflowScheduleTrigger | null>({
    path: `/api/console/applications/${applicationId}/workflow-schedule-trigger`,
    baseUrl
  });
}

export function replaceConsoleWorkflowScheduleTrigger(
  applicationId: string,
  input: ReplaceConsoleWorkflowScheduleTriggerInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleWorkflowScheduleTrigger> {
  return apiFetch<ConsoleWorkflowScheduleTrigger>({
    path: `/api/console/applications/${applicationId}/workflow-schedule-trigger`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function fetchConsoleApplicationApiDocsCatalog(
  applicationId: string,
  baseUrl?: string,
  locale?: string | null
): Promise<ConsoleApiDocsCatalog> {
  return apiFetch<ConsoleApiDocsCatalog>({
    path: buildApplicationApiDocsPath(
      `/api/console/applications/${applicationId}/api-docs/catalog`,
      locale
    ),
    baseUrl
  });
}

export function fetchConsoleApplicationApiDocsCategoryOperations(
  applicationId: string,
  categoryId: string,
  baseUrl?: string,
  locale?: string | null
): Promise<ConsoleApiDocsCategoryOperations>;
export function fetchConsoleApplicationApiDocsCategoryOperations(
  applicationId: string,
  categoryId: string,
  request?: ConsoleApiDocsCategoryOperationsRequest,
  baseUrl?: string,
  locale?: string | null
): Promise<ConsoleApiDocsCategoryOperations>;
export function fetchConsoleApplicationApiDocsCategoryOperations(
  applicationId: string,
  categoryId: string,
  requestOrBaseUrl: ConsoleApiDocsCategoryOperationsRequest | string = {},
  baseUrlOrLocale?: string | null,
  locale?: string | null
): Promise<ConsoleApiDocsCategoryOperations> {
  const request = typeof requestOrBaseUrl === 'string' ? {} : requestOrBaseUrl;
  const baseUrl =
    typeof requestOrBaseUrl === 'string'
      ? requestOrBaseUrl
      : (baseUrlOrLocale ?? undefined);
  const docsLocale =
    typeof requestOrBaseUrl === 'string' ? baseUrlOrLocale : locale;

  return apiFetch<ConsoleApiDocsCategoryOperations>({
    path: buildApplicationApiDocsCategoryOperationsPath(
      `/api/console/applications/${applicationId}/api-docs/categories/${encodeURIComponent(categoryId)}/operations`,
      request,
      docsLocale
    ),
    baseUrl
  });
}

export function fetchConsoleApplicationApiDocsCategorySpec(
  applicationId: string,
  categoryId: string,
  baseUrl?: string,
  locale?: string | null
): Promise<Record<string, unknown>> {
  return apiFetch<Record<string, unknown>>({
    path: buildApplicationApiDocsPath(
      `/api/console/applications/${applicationId}/api-docs/categories/${encodeURIComponent(categoryId)}/openapi.json`,
      locale
    ),
    baseUrl,
    unwrapSuccess: false
  });
}

export function fetchConsoleApplicationApiOperationSpec(
  applicationId: string,
  operationId: string,
  baseUrl?: string,
  locale?: string | null
): Promise<Record<string, unknown>> {
  return apiFetch<Record<string, unknown>>({
    path: buildApplicationApiDocsPath(
      `/api/console/applications/${applicationId}/api-docs/operations/${encodeURIComponent(operationId)}/openapi.json`,
      locale
    ),
    baseUrl,
    unwrapSuccess: false
  });
}
