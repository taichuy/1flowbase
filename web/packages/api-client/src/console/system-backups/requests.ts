import { apiFetch, getDefaultApiBaseUrl } from '../../transport';
import type {
  BackupMutationResponse,
  BackupJobStatusResponse,
  BackupSetDetailResponse,
  BackupSetListResponse,
  BackupVerificationResponse,
  CreateBackupRequest,
  CreateRecoveryIntentRequest,
  QueuedBackupResponse,
  RecoveryIntentResponse,
  RecoveryPreflightResponse,
  RecoveryReauthRequest,
  RecoveryReauthResponse,
  RecoveryStatusResponse
} from './types';

const BASE_PATH = '/api/console/settings/system-backups';
const backupPath = (backupSetId: string) =>
  `${BASE_PATH}/${encodeURIComponent(backupSetId)}`;

export const listSystemBackups = (baseUrl?: string) =>
  apiFetch<BackupSetListResponse>({ path: BASE_PATH, baseUrl });
export const getSystemBackup = (backupSetId: string, baseUrl?: string) =>
  apiFetch<BackupSetDetailResponse>({ path: backupPath(backupSetId), baseUrl });
export const createSystemBackup = (
  csrfToken: string,
  baseUrl?: string,
  request?: CreateBackupRequest
) =>
  apiFetch<QueuedBackupResponse>({
    path: BASE_PATH,
    method: 'POST',
    body: request,
    csrfToken,
    baseUrl
  });
export const getSystemBackupJobStatus = (
  backupJobId: string,
  baseUrl?: string
) =>
  apiFetch<BackupJobStatusResponse>({
    path: `${BASE_PATH}/jobs/status/${encodeURIComponent(backupJobId)}`,
    baseUrl
  });
export const importSystemBackup = (
  file: Blob,
  csrfToken: string,
  baseUrl?: string,
  backupPassword?: string
) =>
  apiFetch<BackupMutationResponse>({
    path: `${BASE_PATH}/import`,
    method: 'POST',
    rawBody: file,
    contentType: 'application/octet-stream',
    headers: backupPassword
      ? { 'x-system-backup-password': backupPassword }
      : undefined,
    csrfToken,
    baseUrl
  });
export const verifySystemBackup = (
  backupSetId: string,
  csrfToken: string,
  baseUrl?: string,
  backupPassword?: string
) =>
  apiFetch<BackupVerificationResponse>({
    path: `${backupPath(backupSetId)}/verify`,
    method: 'POST',
    headers: backupPassword
      ? { 'x-system-backup-password': backupPassword }
      : undefined,
    csrfToken,
    baseUrl
  });
export const deleteSystemBackup = (
  backupSetId: string,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<void>({
    path: backupPath(backupSetId),
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
export const getSystemBackupDownloadUrl = (
  backupSetId: string,
  baseUrl = getDefaultApiBaseUrl()
) => `${baseUrl}${backupPath(backupSetId)}/download`;
export const preflightSystemRecovery = (
  backupSetId: string,
  csrfToken: string,
  baseUrl?: string,
  backupPassword?: string
) =>
  apiFetch<RecoveryPreflightResponse>({
    path: `${backupPath(backupSetId)}/recovery/preflight`,
    method: 'POST',
    headers: backupPassword
      ? { 'x-system-backup-password': backupPassword }
      : undefined,
    csrfToken,
    baseUrl
  });
export const reauthenticateSystemRecovery = (
  request: RecoveryReauthRequest,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<RecoveryReauthResponse>({
    path: `${BASE_PATH}/recovery/reauth`,
    method: 'POST',
    body: request,
    csrfToken,
    baseUrl
  });
export const createSystemRecoveryIntent = (
  backupSetId: string,
  request: CreateRecoveryIntentRequest,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<RecoveryIntentResponse>({
    path: `${backupPath(backupSetId)}/recovery/intents`,
    method: 'POST',
    body: request,
    csrfToken,
    baseUrl
  });
export const getSystemRecoveryStatus = (
  recoveryJobId?: string,
  baseUrl?: string
) => {
  const query = recoveryJobId
    ? `?recovery_job_id=${encodeURIComponent(recoveryJobId)}`
    : '';
  return apiFetch<RecoveryStatusResponse>({
    path: `${BASE_PATH}/recovery/status${query}`,
    baseUrl
  });
};
