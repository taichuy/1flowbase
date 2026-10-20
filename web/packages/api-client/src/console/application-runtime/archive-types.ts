export interface ConsoleApplicationRunArchive {
  archive_version: number;
  exported_at: string;
  manifest: ConsoleApplicationRunArchiveManifest;
  source: ConsoleApplicationRunArchiveSource;
  entries: ConsoleApplicationRunArchiveEntry[];
  content_digest: string;
}

export interface ConsoleApplicationRunArchiveManifest {
  archive_version: number;
  archive_semantics: string;
  exported_at: string;
  source_workspace_id: string;
  source_application_id: string;
  run_count: number;
  selected_run_ids: string[];
  entries: ConsoleApplicationRunArchiveManifestEntry[];
  content_sha256: string;
  checksum: string;
}

export interface ConsoleApplicationRunArchiveManifestEntry {
  source_run_id: string;
  content_sha256: string;
  content_digest: string;
}

export interface ConsoleApplicationRunArchiveSource {
  source_kind: string;
  application_id: string;
  application_type: string;
  application_name: string;
  workspace_id: string;
  exported_by_user_id: string;
  exported_at: string;
  archive_builder: string;
}

export interface ConsoleApplicationRunArchiveFlowRun {
  id: string;
  application_id: string;
  flow_id: string;
  draft_id: string;
  compiled_plan_id: string | null;
  debug_session_id: string;
  flow_schema_version: string;
  document_hash: string;
  run_mode: ConsoleFlowRunMode;
  target_node_id: string | null;
  title: string;
  expand_id: string | null;
  status: string;
  query: string | null;
  model: string | null;
  input_payload: unknown;
  output_payload: unknown;
  error_payload: unknown | null;
  created_by: string;
  authorized_account: string | null;
  api_key_id: string | null;
  publication_version_id: string | null;
  external_user: string | null;
  external_conversation_id: string | null;
  external_trace_id: string | null;
  compatibility_mode: string | null;
  idempotency_key: string | null;
  started_at: string;
  finished_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface ConsoleApplicationRunArchiveNodeRun {
  id: string;
  flow_run_id: string;
  node_id: string;
  node_type: string;
  node_alias: string;
  status: string;
  input_payload: unknown;
  input_payload_view: unknown;
  output_payload: unknown;
  error_payload: unknown | null;
  metrics_payload: unknown;
  debug_payload: unknown;
  started_at: string;
  finished_at: string | null;
}

export interface ConsoleApplicationRunArchiveRunEvent {
  id: string;
  flow_run_id: string;
  node_run_id: string | null;
  sequence: number;
  event_type: string;
  payload: unknown;
  created_at: string;
}

export interface ConsoleApplicationRunArchiveCheckpoint {
  id: string;
  flow_run_id: string;
  node_run_id: string | null;
  status: string;
  reason: string;
  locator_payload: unknown;
  variable_snapshot: unknown;
  external_ref_payload: unknown | null;
  created_at: string;
}

export interface ConsoleApplicationRunArchiveCallbackTask {
  id: string;
  flow_run_id: string;
  node_run_id: string;
  callback_kind: string;
  status: string;
  request_payload: unknown;
  response_payload: unknown | null;
  external_ref_payload: unknown | null;
  created_at: string;
  completed_at: string | null;
}

export interface ConsoleApplicationRunArchiveRuntimeEvent {
  id: string;
  flow_run_id: string;
  node_run_id: string | null;
  span_id: string | null;
  parent_span_id: string | null;
  sequence: number;
  event_type: string;
  layer: string;
  source: string;
  trust_level: string;
  item_id: string | null;
  ledger_ref: string | null;
  payload: unknown;
  visibility: string;
  durability: string;
  created_at: string;
}

export interface ConsoleApplicationRunArchiveUsageLedger {
  id: string;
  flow_run_id: string;
  node_run_id: string | null;
  span_id: string | null;
  failover_attempt_id: string | null;
  provider_instance_id: string | null;
  gateway_route_id: string | null;
  model_id: string | null;
  upstream_model_id: string | null;
  upstream_request_id: string | null;
  input_tokens: number | null;
  cached_input_tokens: number | null;
  output_tokens: number | null;
  reasoning_output_tokens: number | null;
  total_tokens: number | null;
  input_cache_hit_tokens: number | null;
  input_cache_miss_tokens: number | null;
  cache_read_tokens: number | null;
  cache_write_tokens: number | null;
  price_snapshot: unknown | null;
  cost_snapshot: unknown | null;
  usage_status: string;
  raw_usage: unknown;
  normalized_usage: unknown;
  created_at: string;
}

export interface ConsoleApplicationRunArchiveTraceNode extends ConsoleApplicationRunTraceNodeSummary {
  content_kind?: string | null;
  source_refs: unknown;
  detail_refs: unknown;
  payload: unknown;
  children: ConsoleApplicationRunArchiveTraceNode[];
}

export interface ConsoleApplicationRunArchiveTraceTree {
  projection_status: ConsoleApplicationRunTraceProjectionStatus;
  nodes: ConsoleApplicationRunArchiveTraceNode[];
}

export interface ConsoleApplicationRunArchiveRestoreSnapshot {
  flow_run_status: string;
  flow_run_output_payload: unknown;
  flow_run_error_payload: unknown | null;
  latest_checkpoint: ConsoleApplicationRunArchiveCheckpoint | null;
  pending_callback_tasks: ConsoleApplicationRunArchiveCallbackTask[];
}

export interface ConsoleApplicationRunArchiveWarning {
  code: string;
  source: string;
  message: string;
}

export interface ConsoleApplicationRunArchiveEntry {
  source_run_id: string;
  content_digest: string;
  flow_run: ConsoleApplicationRunArchiveFlowRun;
  flow_run_fact: unknown;
  compiled_plan: unknown | null;
  node_runs: ConsoleApplicationRunArchiveNodeRun[];
  events: ConsoleApplicationRunArchiveRunEvent[];
  checkpoints: ConsoleApplicationRunArchiveCheckpoint[];
  callback_tasks: ConsoleApplicationRunArchiveCallbackTask[];
  runtime_spans: unknown[];
  runtime_events: unknown[];
  runtime_items: unknown[];
  context_projections: unknown[];
  usage_ledger: unknown[];
  model_failover_attempts: unknown[];
  capability_invocations: unknown[];
  trace_tree: ConsoleApplicationRunArchiveTraceTree;
  export_warnings: ConsoleApplicationRunArchiveWarning[];
  trace_projection_status?: ConsoleApplicationRunTraceProjectionStatus;
  restore_snapshot?: ConsoleApplicationRunArchiveRestoreSnapshot;
  artifact_materialization_warnings?: ConsoleApplicationRunArchiveWarning[];
}

export interface ConsoleApplicationRunArchiveVersionInput {
  archive_version?: number;
}

export interface ConsoleRunArchiveUploadSessionCreateInput {
  filename?: string | null;
  total_size_bytes: number;
  expected_sha256: string;
  chunk_size_bytes: number;
}

export interface ConsoleRunArchiveUploadSession {
  session_id: string;
  application_id: string;
  status: string;
  filename?: string | null;
  total_size_bytes: number;
  received_bytes: number;
  expected_sha256?: string | null;
  created_at: string;
  updated_at: string;
}

export interface ConsoleRunArchiveChunkUpload {
  session_id: string;
  chunk_index: number;
  chunk_size_bytes: number;
  chunk_sha256: string;
  received_bytes: number;
  status: string;
}

export interface ConsoleRunArchiveImportRunMapping {
  source_run_id: string;
  target_run_id: string;
}

export interface ConsoleRunArchiveImportJob {
  job_id: string;
  application_id: string;
  upload_session_id: string;
  status: string;
  archive_version?: number | null;
  archive_sha256?: string | null;
  run_count: number;
  imported_run_count: number;
  source_to_target_run_ids: ConsoleRunArchiveImportRunMapping[];
  error_payload?: unknown | null;
  result_payload: unknown;
  created_at: string;
  updated_at: string;
  started_at?: string | null;
  finished_at?: string | null;
}
import {
  type ConsoleFlowRunMode,
  type ConsoleApplicationRunTraceNodeSummary,
  type ConsoleApplicationRunTraceProjectionStatus
} from './types';
