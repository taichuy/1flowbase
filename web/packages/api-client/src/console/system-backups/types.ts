export type BackupSetAvailability = 'ready' | 'corrupt' | 'incompatible';

export interface BackupSetSummaryResponse {
  backup_set_id: string;
  exact_backup_name: string;
  created_at: string;
  availability: BackupSetAvailability;
  total_size_bytes: number;
  envelope_digest: string | null;
}

export interface BackupSetListResponse {
  items: BackupSetSummaryResponse[];
}

export interface BackupSetDetailResponse {
  backup_set_id: string;
  exact_backup_name: string;
  sealed_manifest: Record<string, unknown>;
}

export interface BackupMutationResponse {
  backup_set_id: string;
  exact_backup_name: string;
}

export interface BackupVerificationResponse {
  backup_set_id: string;
  verified: boolean;
}

export interface RecoveryImpactPreview {
  database_replaced: boolean;
  business_object_count: number;
  extension_artifact_count: number;
  mcp_artifact_count: number;
  active_work: Array<{
    owner_id: string;
    active_count: number;
    drainable: boolean;
  }>;
}

export interface RecoveryPreflightResponse {
  backup_set_id: string;
  plan_digest: string;
  compatible: boolean;
  required_space_bytes: number;
  available_space_bytes: number;
  impact: RecoveryImpactPreview;
  failures: string[];
}

export interface RecoveryReauthRequest {
  backup_set_id: string;
  exact_backup_name: string;
  plan_digest: string;
  password: string;
}

export interface RecoveryReauthResponse {
  challenge_token: string;
  expires_at: string;
}

export interface CreateRecoveryIntentRequest {
  challenge_token: string;
  exact_backup_name: string;
  plan_digest: string;
}

export interface RecoveryIntentResponse {
  intent_id: string;
  recovery_job_id: string;
  backup_set_id: string;
  status: string;
  expires_at: string;
}

export interface RecoveryStatusResponse {
  phase: string;
  recovery_job_id: string | null;
  active_write_count: number;
  started_at: string | null;
  target_backup_set_id: string | null;
  safety_backup_set_id: string | null;
  plan_digest: string | null;
  journal_state: string | null;
  journal_events: Array<Record<string, unknown>>;
}
