import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';

import { apiFetch, apiFetchBlob, type ApiBlobResponse } from '../transport';
import type {
  ConsoleExtensionRiskChallenge,
  ConsoleExtensionRiskOverride,
  ConsoleExtensionWarning
} from './extensions';

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

export interface ConsoleReferencedI18nMessage {
  key: string;
  text: string;
}

export interface ConsoleApplicationOrchestrationState {
  flow_id: string;
  draft: ConsoleFlowDraftPayload;
  messages: ConsoleReferencedI18nMessage[];
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
  application_type: 'agent_flow' | 'workflow';
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

export interface ExportConsoleApplicationArchiveInput {
  application_ids: string[];
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

export interface ConsoleApplicationArchivePreview {
  applications: Array<{
    entry_index: number;
    preview: ConsoleAgentFlowTemplatePreview;
  }>;
}

export interface ImportConsoleApplicationArchiveSelection {
  entry_index: number;
  name?: string;
  description?: string;
}

export type ConsoleApplicationArchiveImportResult =
  | {
      entry_index: number;
      status: 'succeeded';
      result: ImportConsoleAgentFlowTemplateResponse;
    }
  | { entry_index: number; status: 'failed'; code: string }
  | {
      entry_index: number;
      status: 'partial';
      application_id: string;
      code: string;
    };

export interface ImportConsoleApplicationArchiveResponse {
  results: ConsoleApplicationArchiveImportResult[];
  succeeded_count: number;
  failed_count: number;
  partial_count: number;
}

export interface PreviewConsoleAgentFlowTemplateInput {
  template: ConsoleAgentFlowTemplatePackage;
}

export interface ImportConsoleAgentFlowTemplateInput {
  template: ConsoleAgentFlowTemplatePackage;
  name?: string;
  description?: string;
}

export interface ImportConsoleApplicationArchiveInput {
  applications?: ImportConsoleApplicationArchiveSelection[];
  file: Blob;
  filename?: string;
  name?: string;
  description?: string;
}

export interface ConsoleAgentFlowTemplateImportedApplication {
  id: string;
  application_type: 'agent_flow' | 'workflow';
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

export function exportConsoleApplicationArchive(
  input: ExportConsoleApplicationArchiveInput,
  baseUrl?: string
): Promise<ApiBlobResponse> {
  return apiFetchBlob({
    path: '/api/console/applications/archive/export',
    method: 'POST',
    body: input,
    baseUrl
  });
}

export function previewConsoleApplicationArchive(
  file: Blob,
  filename = 'application.zip',
  baseUrl?: string
): Promise<ConsoleApplicationArchivePreview> {
  const formData = new FormData();
  formData.append('file', file, filename);
  return apiFetch<ConsoleApplicationArchivePreview>({
    path: '/api/console/applications/archive/preview',
    method: 'POST',
    rawBody: formData,
    baseUrl
  });
}

export function importConsoleApplicationArchive(
  input: ImportConsoleApplicationArchiveInput,
  csrfToken: string,
  baseUrl?: string
): Promise<ImportConsoleApplicationArchiveResponse> {
  const formData = new FormData();
  formData.append('file', input.file, input.filename ?? 'application.zip');
  if (input.applications)
    formData.append('applications', JSON.stringify(input.applications));
  if (input.name) formData.append('name', input.name);
  if (input.description !== undefined) {
    formData.append('description', input.description);
  }
  return apiFetch<ImportConsoleApplicationArchiveResponse>({
    path: '/api/console/applications/archive/import',
    method: 'POST',
    rawBody: formData,
    csrfToken,
    baseUrl
  });
}

export interface ConsoleInstalledApplicationExtensionPreview {
  extension_installation_id: string;
  application_status: 'not_applied' | 'applied';
  integrity_warnings: ConsoleExtensionWarning[];
  required_integrity_override: ConsoleExtensionRiskChallenge | null;
  preview: ConsoleAgentFlowTemplatePreview;
}

export function previewConsoleInstalledApplicationExtension(
  extensionInstallationId: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleInstalledApplicationExtensionPreview>({
    path: `/api/console/applications/archive/installed-extension/${encodeURIComponent(
      extensionInstallationId
    )}/preview`,
    baseUrl
  });
}

export function importConsoleInstalledApplicationExtension(
  extensionInstallationId: string,
  input: {
    name?: string;
    description?: string;
    integrity_override?: ConsoleExtensionRiskOverride;
  },
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ImportConsoleAgentFlowTemplateResponse>({
    path: `/api/console/applications/archive/installed-extension/${encodeURIComponent(
      extensionInstallationId
    )}/import`,
    method: 'POST',
    body: input,
    csrfToken,
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
