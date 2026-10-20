import {
  type ConsoleCallbackTask,
  type ConsoleNodeRunDetail,
  type ConsoleRunCheckpoint,
  type ConsoleRunEvent
} from '@1flowbase/api-client';

function stringField(record: Record<string, unknown>, field: string): string {
  const value = record[field];

  if (typeof value !== 'string') {
    throw new Error(`invalid_${field}`);
  }

  return value;
}

function optionalStringField(
  record: Record<string, unknown>,
  field: string
): string | null {
  const value = record[field];

  if (value === null || value === undefined) {
    return null;
  }

  if (typeof value !== 'string') {
    throw new Error(`invalid_${field}`);
  }

  return value;
}

function stringArrayField(
  record: Record<string, unknown>,
  field: string
): string[] {
  const value = record[field];

  if (!Array.isArray(value) || value.some((item) => typeof item !== 'string')) {
    throw new Error(`invalid_${field}`);
  }

  return value as string[];
}

function numberField(record: Record<string, unknown>, field: string): number {
  const value = record[field];

  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new Error(`invalid_${field}`);
  }

  return value;
}

function optionalNumberField(
  record: Record<string, unknown>,
  field: string
): number | null {
  const value = record[field];

  if (value === null || value === undefined) {
    return null;
  }

  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new Error(`invalid_${field}`);
  }

  return value;
}

function recordPayload(
  record: Record<string, unknown>,
  field: string
): Record<string, unknown> {
  const value = record[field];

  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`invalid_${field}`);
  }

  return value as Record<string, unknown>;
}

function optionalRecordPayload(
  record: Record<string, unknown>,
  field: string
): Record<string, unknown> | null {
  const value = record[field];

  if (value === null || value === undefined) {
    return null;
  }

  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`invalid_${field}`);
  }

  return value as Record<string, unknown>;
}

export function toApplicationRunSummary(
  record: Record<string, unknown>
): ApplicationRunSummary {
  const id = stringField(record, 'id');

  return {
    source_kind: optionalStringField(
      record,
      'source_kind'
    ) as ApplicationRunSummary['source_kind'],
    source_client: optionalStringField(record, 'source_client'),
    source_session_id: optionalStringField(record, 'source_session_id'),
    source_task_id: optionalStringField(record, 'source_task_id'),
    native_run_id: optionalStringField(record, 'native_run_id'),
    id,
    application_id: stringField(record, 'application_id'),
    scope_id: stringField(record, 'scope_id'),
    log_conversation_id: optionalStringField(record, 'log_conversation_id'),
    log_task_run_id: optionalStringField(record, 'log_task_run_id'),
    parent_run_id: optionalStringField(record, 'parent_run_id'),
    caused_by_run_id: optionalStringField(record, 'caused_by_run_id'),
    call_kind: stringField(record, 'call_kind'),
    invocation_count: numberField(record, 'invocation_count'),
    compaction_count: numberField(record, 'compaction_count'),
    member_run_ids: stringArrayField(record, 'member_run_ids'),
    parent_task_run_id: optionalStringField(record, 'parent_task_run_id'),
    outcome: stringField(record, 'outcome'),
    user_input: optionalStringField(record, 'user_input'),
    final_output: optionalStringField(record, 'final_output'),
    run_mode: stringField(
      record,
      'run_mode'
    ) as ApplicationRunSummary['run_mode'],
    execution_stage: stringField(
      record,
      'execution_stage'
    ) as ApplicationRunSummary['execution_stage'],
    invocation_source: stringField(
      record,
      'invocation_source'
    ) as ApplicationRunSummary['invocation_source'],
    principal: recordPayload(
      record,
      'principal'
    ) as unknown as ApplicationRunSummary['principal'],
    status: stringField(record, 'status'),
    target_node_id: optionalStringField(record, 'target_node_id'),
    title: stringField(record, 'title'),
    expand_id: optionalStringField(record, 'expand_id'),
    external_user: optionalStringField(record, 'external_user'),
    authorized_account: optionalStringField(record, 'authorized_account'),
    api_key_id: optionalStringField(record, 'api_key_id'),
    api_key_name_snapshot: optionalStringField(record, 'api_key_name_snapshot'),
    requested_model_id: optionalStringField(record, 'requested_model_id'),
    reasoning_effort: optionalStringField(record, 'reasoning_effort'),
    publication_version_id: optionalStringField(
      record,
      'publication_version_id'
    ),
    external_conversation_id: optionalStringField(
      record,
      'external_conversation_id'
    ),
    external_trace_id: optionalStringField(record, 'external_trace_id'),
    compatibility_mode: optionalStringField(record, 'compatibility_mode'),
    idempotency_key: optionalStringField(record, 'idempotency_key'),
    total_cost: optionalNumberField(record, 'total_cost'),
    total_tokens: optionalNumberField(record, 'total_tokens'),
    input_tokens: optionalNumberField(record, 'input_tokens'),
    output_tokens: optionalNumberField(record, 'output_tokens'),
    count_tokens_input_tokens: optionalNumberField(
      record,
      'count_tokens_input_tokens'
    ),
    input_cache_hit_tokens: optionalNumberField(
      record,
      'input_cache_hit_tokens'
    ),
    input_cache_hit_rate: optionalNumberField(record, 'input_cache_hit_rate'),
    unique_node_count: numberField(record, 'unique_node_count'),
    tool_callback_count: numberField(record, 'tool_callback_count'),
    started_at: stringField(record, 'started_at'),
    finished_at: optionalStringField(record, 'finished_at'),
    created_at: stringField(record, 'created_at'),
    updated_at: stringField(record, 'updated_at')
  };
}

export function toApplicationConversation(
  record: Record<string, unknown>
): ApplicationConversationRecord {
  return {
    id: stringField(record, 'id'),
    application_id: stringField(record, 'application_id'),
    scope_id: stringField(record, 'scope_id'),
    external_conversation_id: stringField(record, 'external_conversation_id'),
    external_user: optionalStringField(record, 'external_user'),
    api_key_id: optionalStringField(record, 'api_key_id'),
    created_at: stringField(record, 'created_at'),
    updated_at: stringField(record, 'updated_at')
  };
}

function conversationMessageRole(
  record: Record<string, unknown>
): ApplicationConversationMessageRecord['role'] {
  const role = stringField(record, 'role');

  if (role !== 'system' && role !== 'user' && role !== 'assistant') {
    throw new Error('invalid_role');
  }

  return role;
}

export function toApplicationConversationMessage(
  record: Record<string, unknown>
): ApplicationConversationMessageRecord {
  return {
    id: stringField(record, 'id'),
    application_id: stringField(record, 'application_id'),
    scope_id: stringField(record, 'scope_id'),
    conversation_id: optionalStringField(record, 'conversation_id'),
    flow_run_id: optionalStringField(record, 'flow_run_id'),
    role: conversationMessageRole(record),
    content: stringField(record, 'content'),
    sequence: numberField(record, 'sequence'),
    status: stringField(record, 'status'),
    created_at: stringField(record, 'created_at'),
    started_at: optionalStringField(record, 'started_at'),
    finished_at: optionalStringField(record, 'finished_at')
  };
}

export function toApplicationRunNodeRun(
  record: Record<string, unknown>
): ConsoleNodeRunDetail {
  return {
    id: stringField(record, 'id'),
    flow_run_id: stringField(record, 'flow_run_id'),
    node_id: stringField(record, 'node_id'),
    node_type: stringField(record, 'node_type'),
    node_alias: stringField(record, 'node_alias'),
    status: stringField(record, 'status'),
    input_payload: recordPayload(record, 'input_payload'),
    input_payload_view:
      optionalRecordPayload(record, 'input_payload_view') ?? undefined,
    output_payload: recordPayload(record, 'output_payload'),
    error_payload: optionalRecordPayload(record, 'error_payload'),
    metrics_payload: recordPayload(record, 'metrics_payload'),
    debug_payload: optionalRecordPayload(record, 'debug_payload') ?? undefined,
    started_at: stringField(record, 'started_at'),
    finished_at: optionalStringField(record, 'finished_at')
  };
}

export function toApplicationRunEvent(
  record: Record<string, unknown>
): ConsoleRunEvent {
  return {
    id: stringField(record, 'id'),
    flow_run_id: stringField(record, 'flow_run_id'),
    node_run_id: optionalStringField(record, 'node_run_id'),
    sequence: numberField(record, 'sequence'),
    event_type: stringField(record, 'event_type'),
    payload: recordPayload(record, 'payload'),
    created_at: stringField(record, 'created_at')
  };
}

export function toApplicationRunCheckpoint(
  record: Record<string, unknown>
): ConsoleRunCheckpoint {
  return {
    id: stringField(record, 'id'),
    flow_run_id: stringField(record, 'flow_run_id'),
    node_run_id: optionalStringField(record, 'node_run_id'),
    status: stringField(record, 'status'),
    reason: stringField(record, 'reason'),
    locator_payload: recordPayload(record, 'locator_payload'),
    variable_snapshot: recordPayload(record, 'variable_snapshot'),
    external_ref_payload: optionalRecordPayload(record, 'external_ref_payload'),
    created_at: stringField(record, 'created_at')
  };
}

export function toApplicationRunCallbackTask(
  record: Record<string, unknown>
): ConsoleCallbackTask {
  return {
    id: stringField(record, 'id'),
    flow_run_id: stringField(record, 'flow_run_id'),
    node_run_id: stringField(record, 'node_run_id'),
    callback_kind: stringField(record, 'callback_kind'),
    status: stringField(record, 'status') as ConsoleCallbackTask['status'],
    request_payload: recordPayload(record, 'request_payload'),
    response_payload: optionalRecordPayload(record, 'response_payload'),
    external_ref_payload: optionalRecordPayload(record, 'external_ref_payload'),
    created_at: stringField(record, 'created_at'),
    completed_at: optionalStringField(record, 'completed_at')
  };
}
import {
  type ApplicationRunSummary,
  type ApplicationConversationRecord,
  type ApplicationConversationMessageRecord
} from './runtime';
