create index if not exists runtime_events_flow_callback_sequence_idx
    on runtime_events (flow_run_id, (payload ->> 'callback_task_id'), sequence desc)
    where payload ? 'callback_task_id';
