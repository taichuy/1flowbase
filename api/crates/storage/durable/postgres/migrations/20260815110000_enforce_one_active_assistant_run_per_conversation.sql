create unique index if not exists uq_flow_runs_active_assistant_conversation
on flow_runs (assistant_conversation_id)
where assistant_conversation_id is not null
  and run_mode = 'assistant_execution'
  and compatibility_mode = 'embedded_assistant'
  and status in (
      'queued',
      'running',
      'waiting_callback',
      'waiting_human',
      'paused'
  );
