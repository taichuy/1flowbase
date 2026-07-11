import type {
  ConsoleWorkflowExtensionHttpMethod,
  ConsoleWorkflowExtensionResponseMode
} from '../application-public-api';
import { apiFetch, apiFetchVoid } from '../transport';

export type ConsoleApplicationType = 'agent_flow' | 'workflow';
export type ConsoleWorkflowTriggerType = 'extension' | 'schedule';
export type CreateConsoleWorkflowTriggerConfig =
  | {
      cron: string;
      timezone: string;
      input_payload: unknown;
    }
  | {
      subpath: string;
      http_method: ConsoleWorkflowExtensionHttpMethod;
      response_mode: ConsoleWorkflowExtensionResponseMode;
    };

export interface ConsoleApplicationTag {
  id: string;
  name: string;
}

export interface ConsoleApplicationTagCatalogEntry extends ConsoleApplicationTag {
  application_count: number;
}

export interface ConsoleApplicationTypeOption {
  value: ConsoleApplicationType;
  label: string;
}

export interface ConsoleApplicationCatalog {
  types: ConsoleApplicationTypeOption[];
  tags: ConsoleApplicationTagCatalogEntry[];
}

export interface ConsoleApplicationSummary {
  id: string;
  application_type: ConsoleApplicationType;
  name: string;
  description: string;
  icon: string | null;
  icon_type: string | null;
  icon_background: string | null;
  created_by: string;
  updated_at: string;
  tags: ConsoleApplicationTag[];
}

export interface ConsoleApplicationSections {
  orchestration: {
    status: string;
    subject_kind: string;
    subject_status: string;
    current_subject_id: string | null;
    current_draft_id: string | null;
  };
  api: {
    status: string;
    credential_kind: string;
    invoke_routing_mode: string;
    invoke_path_template: string | null;
    api_capability_status: string;
    credentials_status: string;
  };
  logs: {
    status: string;
    runs_capability_status: string;
    run_object_kind: string;
    log_retention_status: string;
  };
  monitoring: {
    status: string;
    metrics_capability_status: string;
    metrics_object_kind: string;
    tracing_config_status: string;
  };
}

export interface ConsoleApplicationDetail extends ConsoleApplicationSummary {
  workflow_trigger_type: ConsoleWorkflowTriggerType | null;
  sections: ConsoleApplicationSections;
}

export interface ConsoleApplicationEnvironmentVariable {
  name: string;
  value_type: string;
  value: unknown;
  description: string;
  updated_at: string;
}

export interface CreateConsoleApplicationInput {
  application_type: ConsoleApplicationType;
  workflow_trigger_type?: ConsoleWorkflowTriggerType | null;
  workflow_trigger_config?: CreateConsoleWorkflowTriggerConfig | null;
  name: string;
  description: string;
  icon: string | null;
  icon_type: string | null;
  icon_background: string | null;
}

export interface UpdateConsoleApplicationInput {
  name: string;
  description: string;
  tag_ids: string[];
}

export interface CreateConsoleApplicationTagInput {
  name: string;
}

export interface ReplaceConsoleApplicationEnvironmentVariablesInput {
  variables: Array<{
    name: string;
    value_type: string;
    value: unknown;
    description: string;
  }>;
}

export function listConsoleApplications(
  baseUrl?: string
): Promise<ConsoleApplicationSummary[]> {
  return apiFetch<ConsoleApplicationSummary[]>({
    path: '/api/console/applications',
    baseUrl
  });
}

export function getConsoleApplicationCatalog(
  baseUrl?: string
): Promise<ConsoleApplicationCatalog> {
  return apiFetch<ConsoleApplicationCatalog>({
    path: '/api/console/applications/catalog',
    baseUrl
  });
}

export function createConsoleApplication(
  input: CreateConsoleApplicationInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleApplicationDetail> {
  return apiFetch<ConsoleApplicationDetail>({
    path: '/api/console/applications',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function getConsoleApplication(
  applicationId: string,
  baseUrl?: string
): Promise<ConsoleApplicationDetail> {
  return apiFetch<ConsoleApplicationDetail>({
    path: `/api/console/applications/${applicationId}`,
    baseUrl
  });
}

export function updateConsoleApplication(
  applicationId: string,
  input: UpdateConsoleApplicationInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleApplicationDetail> {
  return apiFetch<ConsoleApplicationDetail>({
    path: `/api/console/applications/${applicationId}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleApplication(
  applicationId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<void> {
  return apiFetchVoid({
    path: `/api/console/applications/${applicationId}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function listConsoleApplicationEnvironmentVariables(
  applicationId: string,
  baseUrl?: string
): Promise<ConsoleApplicationEnvironmentVariable[]> {
  return apiFetch<ConsoleApplicationEnvironmentVariable[]>({
    path: `/api/console/applications/${applicationId}/environment-variables`,
    baseUrl
  });
}

export function replaceConsoleApplicationEnvironmentVariables(
  applicationId: string,
  input: ReplaceConsoleApplicationEnvironmentVariablesInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleApplicationEnvironmentVariable[]> {
  return apiFetch<ConsoleApplicationEnvironmentVariable[]>({
    path: `/api/console/applications/${applicationId}/environment-variables`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function createConsoleApplicationTag(
  input: CreateConsoleApplicationTagInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleApplicationTagCatalogEntry> {
  return apiFetch<ConsoleApplicationTagCatalogEntry>({
    path: '/api/console/applications/tags',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}
