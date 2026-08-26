alter table flow_runs
    drop constraint if exists flow_runs_status_check;

alter table flow_runs
    add constraint flow_runs_status_check
    check (
        status in (
            'queued',
            'running',
            'waiting_callback',
            'waiting_human',
            'paused',
            'succeeded',
            'incomplete',
            'failed',
            'cancelled'
        )
    );

alter table application_run_log_summaries
    drop constraint if exists application_run_log_summaries_status_check;

alter table application_run_log_summaries
    add constraint application_run_log_summaries_status_check
    check (
        status in (
            'queued',
            'running',
            'waiting_callback',
            'waiting_human',
            'paused',
            'succeeded',
            'incomplete',
            'failed',
            'cancelled'
        )
    );
