-- Runtime log deletion traverses CASCADE, SET NULL and RESTRICT edges per parent.
-- These leading equality indexes bound each probe to matching rows instead of
-- rescanning retained history. Existing FK actions and financial retention stay unchanged.
-- Nullable keys use IS NOT NULL; every equality FK probe implies this predicate.
-- The migration is transactional and runs before background deletion workers start.

create index cleanup_application_run_conversation_message_items_det_558ddd8d
    on application_run_conversation_message_items (detail_run_id) where detail_run_id is not null;

create index cleanup_application_run_log_summaries_caused_by_run_id
    on application_run_log_summaries (caused_by_run_id) where caused_by_run_id is not null;

create index cleanup_application_run_log_summaries_log_task_run_id
    on application_run_log_summaries (log_task_run_id) where log_task_run_id is not null;

create index cleanup_application_run_log_summaries_parent_run_id
    on application_run_log_summaries (parent_run_id) where parent_run_id is not null;

create index cleanup_application_run_log_tasks_final_output_run_id
    on application_run_log_tasks (final_output_run_id) where final_output_run_id is not null;

create index cleanup_application_run_log_tasks_native_run_id
    on application_run_log_tasks (native_run_id) where native_run_id is not null;

create index cleanup_application_run_trace_nodes_parent_trace_node_id
    on application_run_trace_nodes (parent_trace_node_id) where parent_trace_node_id is not null;

create index cleanup_assistant_conversations_seed_legacy_flow_run_id
    on assistant_conversations (seed_legacy_flow_run_id) where seed_legacy_flow_run_id is not null;

create index cleanup_capability_invocations_requested_by_span_id
    on capability_invocations (requested_by_span_id) where requested_by_span_id is not null;

create index cleanup_capability_invocations_span_id
    on capability_invocations (span_id) where span_id is not null;

create index cleanup_client_trajectory_archive_heads_flow_run_id
    on client_trajectory_archive_heads (flow_run_id) where flow_run_id is not null;

create index cleanup_client_trajectory_captures_event_id
    on client_trajectory_captures (event_id) where event_id is not null;

create index cleanup_client_trajectory_node_links_event_id
    on client_trajectory_node_links (event_id);

create index cleanup_client_trajectory_node_links_node_run_id
    on client_trajectory_node_links (node_run_id);

create index cleanup_data_model_side_effect_receipts_node_run_id
    on data_model_side_effect_receipts (node_run_id);

create index cleanup_flow_run_callback_tasks_node_run_id
    on flow_run_callback_tasks (node_run_id);

create index cleanup_flow_run_checkpoints_flow_run_id
    on flow_run_checkpoints (flow_run_id);

create index cleanup_flow_run_checkpoints_node_run_id
    on flow_run_checkpoints (node_run_id) where node_run_id is not null;

create index cleanup_flow_run_events_node_run_id
    on flow_run_events (node_run_id) where node_run_id is not null;

create index cleanup_flow_run_recovery_history_context_version_id
    on flow_run_recovery_history (context_version_id);

create index cleanup_flow_run_recovery_history_node_run_id
    on flow_run_recovery_history (node_run_id) where node_run_id is not null;

create index cleanup_flow_run_tool_callback_inbox_flow_run_id
    on flow_run_tool_callback_inbox (flow_run_id);

create index cleanup_model_failover_attempt_ledger_llm_turn_span_id
    on model_failover_attempt_ledger (llm_turn_span_id) where llm_turn_span_id is not null;

create index cleanup_model_failover_attempt_ledger_node_run_id
    on model_failover_attempt_ledger (node_run_id) where node_run_id is not null;

create index cleanup_native_trajectory_integrity_event_id
    on native_trajectory_integrity (event_id);

create index cleanup_runtime_artifacts_flow_run_id
    on runtime_artifacts (flow_run_id);

create index cleanup_runtime_audit_hashes_flow_run_id
    on runtime_audit_hashes (flow_run_id);

create index cleanup_runtime_context_projections_compaction_event_id
    on runtime_context_projections (compaction_event_id) where compaction_event_id is not null;

create index cleanup_runtime_context_projections_llm_turn_span_id
    on runtime_context_projections (llm_turn_span_id) where llm_turn_span_id is not null;

create index cleanup_runtime_context_projections_node_run_id
    on runtime_context_projections (node_run_id) where node_run_id is not null;

create index cleanup_runtime_context_projections_previous_projection_id
    on runtime_context_projections (previous_projection_id) where previous_projection_id is not null;

create index cleanup_runtime_cost_ledger_span_id
    on runtime_cost_ledger (span_id) where span_id is not null;

create index cleanup_runtime_credit_ledger_span_id
    on runtime_credit_ledger (span_id) where span_id is not null;

create index cleanup_runtime_debug_artifacts_run_event_id
    on runtime_debug_artifacts (run_event_id) where run_event_id is not null;

create index cleanup_runtime_invocation_context_bindings_context_version_id
    on runtime_invocation_context_bindings (context_version_id);

create index cleanup_runtime_invocation_context_bindings_flow_run_id
    on runtime_invocation_context_bindings (flow_run_id);

create index cleanup_runtime_items_flow_run_id
    on runtime_items (flow_run_id);

create index cleanup_runtime_items_source_event_id
    on runtime_items (source_event_id) where source_event_id is not null;

create index cleanup_runtime_items_span_id
    on runtime_items (span_id) where span_id is not null;

create index cleanup_runtime_legacy_shadow_rows_context_version_id
    on runtime_legacy_shadow_rows (context_version_id) where context_version_id is not null;

create index cleanup_runtime_legacy_shadow_rows_flow_run_id
    on runtime_legacy_shadow_rows (flow_run_id);

create index cleanup_runtime_spans_node_run_id
    on runtime_spans (node_run_id) where node_run_id is not null;

create index cleanup_runtime_spans_parent_span_id
    on runtime_spans (parent_span_id) where parent_span_id is not null;

create index cleanup_runtime_usage_ledger_failover_attempt_id
    on runtime_usage_ledger (failover_attempt_id) where failover_attempt_id is not null;

create index cleanup_runtime_usage_ledger_node_run_id
    on runtime_usage_ledger (node_run_id) where node_run_id is not null;

create index cleanup_runtime_usage_ledger_span_id
    on runtime_usage_ledger (span_id) where span_id is not null;
