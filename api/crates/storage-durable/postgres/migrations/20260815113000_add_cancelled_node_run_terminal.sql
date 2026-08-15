alter table node_runs
    drop constraint node_runs_status_check;

alter table node_runs
    add constraint node_runs_status_check check (
        status in (
            'pending',
            'ready',
            'running',
            'streaming',
            'waiting_tool',
            'waiting_callback',
            'waiting_human',
            'retrying',
            'succeeded',
            'failed',
            'cancelled',
            'skipped'
        )
    );
