import type { FlowAuthoringDocument } from '@1flowbase/flow-schema';
import {
  analyzeConsoleApplicationDocument,
  getConsoleApplicationOrchestration,
  restoreConsoleApplicationVersion,
  saveConsoleApplicationDraft,
  updateConsoleApplicationVersion,
  type SaveConsoleApplicationDraftInput,
  type UpdateConsoleApplicationVersionInput
} from '@1flowbase/api-client';

import { getApplicationsApiBaseUrl } from '../../applications/api/applications';

export const orchestrationQueryKey = (applicationId: string) =>
  ['applications', applicationId, 'orchestration'] as const;

export function fetchOrchestrationState(applicationId: string) {
  return getConsoleApplicationOrchestration(
    applicationId,
    getApplicationsApiBaseUrl()
  );
}

export function saveDraft(
  applicationId: string,
  input: SaveConsoleApplicationDraftInput,
  csrfToken: string
) {
  return saveConsoleApplicationDraft(
    applicationId,
    input,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function restoreVersion(
  applicationId: string,
  versionId: string,
  csrfToken: string
) {
  return restoreConsoleApplicationVersion(
    applicationId,
    versionId,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function updateVersion(
  applicationId: string,
  versionId: string,
  input: UpdateConsoleApplicationVersionInput,
  csrfToken: string
) {
  return updateConsoleApplicationVersion(
    applicationId,
    versionId,
    input,
    csrfToken,
    getApplicationsApiBaseUrl()
  );
}

export function analyzeDocument(
  applicationId: string,
  document: FlowAuthoringDocument,
  signal?: AbortSignal,
  node_id?: string
) {
  return analyzeConsoleApplicationDocument(
    applicationId,
    { document, ...(node_id ? { node_id } : {}) },
    getApplicationsApiBaseUrl(),
    signal
  );
}
