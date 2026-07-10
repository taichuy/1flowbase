import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

import { apiFetch } from '../transport';

export interface ConsoleFlowVersionSummary {
  id: string;
  sequence: number;
  trigger: 'autosave' | 'restore';
  change_kind: 'logical';
  summary: string;
  summary_is_custom?: boolean;
  is_user_protected: boolean;
  is_current_publication: boolean;
  created_at: string;
}

export interface ConsoleFlowDraftPayload {
  id: string;
  flow_id: string;
  document: FlowAuthoringDocument;
  updated_at: string;
}

export interface ConsoleApplicationOrchestrationState {
  flow_id: string;
  draft: ConsoleFlowDraftPayload;
  versions: ConsoleFlowVersionSummary[];
  autosave_interval_seconds: number;
  user_protection_limit: number;
}

export interface SaveConsoleApplicationDraftInput {
  document: FlowAuthoringDocument;
  change_kind: 'layout' | 'logical';
  summary: string;
}

export interface UpdateConsoleApplicationVersionInput {
  summary?: string;
  summary_is_custom?: boolean;
  is_user_protected?: boolean;
}

export interface ConsoleAgentFlowTemplateApplication {
  application_type: 'agent_flow';
  name: string;
  description: string;
  icon: string | null;
  icon_type: string | null;
  icon_background: string | null;
}

export interface ConsoleAgentFlowTemplateDependency {
  kind: string;
  node_id: string | null;
  node_type: string | null;
  config_version: number | null;
  provider_code: string | null;
  model_id: string | null;
  plugin_id: string | null;
  plugin_version: string | null;
  contribution_code: string | null;
  node_shell: string | null;
  schema_version: string | null;
  plugin_unique_identifier: string | null;
  package_id: string | null;
  contribution_checksum: string | null;
  compiled_contribution_hash: string | null;
}

export interface ConsoleAgentFlowTemplatePackage {
  schema_version: '1flowbase.application-template/v1';
  application: ConsoleAgentFlowTemplateApplication;
  flow_document: FlowAuthoringDocument;
  dependencies: ConsoleAgentFlowTemplateDependency[];
}

export interface ConsoleOfficialAgentFlowTemplateCatalogSource {
  source_kind: string;
  source_label: string;
  index_url: string;
}

export interface ConsoleOfficialAgentFlowTemplateCatalogPage {
  page: number;
  page_size: number;
  next_cursor: string | null;
}

export interface ConsoleOfficialAgentFlowTemplateCatalogEntry {
  workflow_id: string;
  schema_version: string;
  application: ConsoleAgentFlowTemplateApplication;
  template_url: string;
  template_sha256: string;
  updated_at: string;
}

export interface ConsoleOfficialAgentFlowTemplateCatalog {
  source: ConsoleOfficialAgentFlowTemplateCatalogSource;
  page: ConsoleOfficialAgentFlowTemplateCatalogPage;
  entries: ConsoleOfficialAgentFlowTemplateCatalogEntry[];
}

export interface ListConsoleOfficialAgentFlowTemplateCatalogRequest {
  cursor?: string | null;
}

export interface ConsoleAgentFlowTemplateDependencyStatus {
  dependency: ConsoleAgentFlowTemplateDependency;
  status: string;
  reason: string | null;
}

export interface ConsoleAgentFlowTemplateUnresolvedNode {
  node_id: string;
  alias: string;
  original_type: string;
  dependency_status: string;
  reason: string;
  original_node: Record<string, unknown>;
}

export interface ConsoleAgentFlowTemplatePreview {
  schema_version: '1flowbase.application-template/v1';
  application: ConsoleAgentFlowTemplateApplication;
  dependencies: ConsoleAgentFlowTemplateDependencyStatus[];
  unresolved_nodes: ConsoleAgentFlowTemplateUnresolvedNode[];
  document: FlowAuthoringDocument;
}

export interface PreviewConsoleAgentFlowTemplateInput {
  template: ConsoleAgentFlowTemplatePackage;
}

export interface ImportConsoleAgentFlowTemplateInput {
  template: ConsoleAgentFlowTemplatePackage;
  name?: string;
  description?: string;
}

export interface ConsoleAgentFlowTemplateImportedApplication {
  id: string;
  application_type: 'agent_flow';
  name: string;
  description: string;
  icon: string | null;
  icon_type: string | null;
  icon_background: string | null;
  created_by: string;
  updated_at: string;
}

export interface ImportConsoleAgentFlowTemplateResponse {
  application: ConsoleAgentFlowTemplateImportedApplication;
  orchestration: ConsoleApplicationOrchestrationState;
  preview: ConsoleAgentFlowTemplatePreview;
}

export function getConsoleApplicationOrchestration(
  applicationId: string,
  baseUrl?: string
): Promise<ConsoleApplicationOrchestrationState> {
  return apiFetch<ConsoleApplicationOrchestrationState>({
    path: `/api/console/applications/${applicationId}/orchestration`,
    baseUrl
  });
}

export function saveConsoleApplicationDraft(
  applicationId: string,
  input: SaveConsoleApplicationDraftInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleApplicationOrchestrationState> {
  return apiFetch<ConsoleApplicationOrchestrationState>({
    path: `/api/console/applications/${applicationId}/orchestration/draft`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function exportConsoleAgentFlowTemplate(
  applicationId: string,
  baseUrl?: string
): Promise<ConsoleAgentFlowTemplatePackage> {
  return apiFetch<ConsoleAgentFlowTemplatePackage>({
    path: `/api/console/applications/${applicationId}/orchestration/template`,
    baseUrl
  });
}

export function previewConsoleAgentFlowTemplate(
  input: PreviewConsoleAgentFlowTemplateInput,
  baseUrl?: string
): Promise<ConsoleAgentFlowTemplatePreview> {
  return apiFetch<ConsoleAgentFlowTemplatePreview>({
    path: '/api/console/applications/orchestration/template/preview',
    method: 'POST',
    body: input,
    baseUrl
  });
}

export function importConsoleAgentFlowTemplate(
  input: ImportConsoleAgentFlowTemplateInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ImportConsoleAgentFlowTemplateResponse> {
  return apiFetch<ImportConsoleAgentFlowTemplateResponse>({
    path: '/api/console/applications/orchestration/template/import',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function listConsoleOfficialAgentFlowTemplateCatalog(
  request: ListConsoleOfficialAgentFlowTemplateCatalogRequest = {},
  baseUrl?: string
): Promise<ConsoleOfficialAgentFlowTemplateCatalog> {
  const params = new URLSearchParams();

  if (request.cursor) {
    params.set('cursor', request.cursor);
  }

  const query = params.size > 0 ? `?${params.toString()}` : '';

  return apiFetch<ConsoleOfficialAgentFlowTemplateCatalog>({
    path: `/api/console/applications/orchestration/templates/official-catalog${query}`,
    baseUrl
  });
}

export function downloadConsoleOfficialAgentFlowTemplate(
  workflowId: string,
  baseUrl?: string
): Promise<ConsoleAgentFlowTemplatePackage> {
  return apiFetch<ConsoleAgentFlowTemplatePackage>({
    path: `/api/console/applications/orchestration/templates/official/${encodeURIComponent(
      workflowId
    )}`,
    baseUrl
  });
}

export function restoreConsoleApplicationVersion(
  applicationId: string,
  versionId: string,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleApplicationOrchestrationState> {
  return apiFetch<ConsoleApplicationOrchestrationState>({
    path: `/api/console/applications/${applicationId}/orchestration/versions/${versionId}/restore`,
    method: 'POST',
    csrfToken,
    baseUrl
  });
}

export function updateConsoleApplicationVersion(
  applicationId: string,
  versionId: string,
  input: UpdateConsoleApplicationVersionInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ConsoleApplicationOrchestrationState> {
  return apiFetch<ConsoleApplicationOrchestrationState>({
    path: `/api/console/applications/${applicationId}/orchestration/versions/${versionId}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}
