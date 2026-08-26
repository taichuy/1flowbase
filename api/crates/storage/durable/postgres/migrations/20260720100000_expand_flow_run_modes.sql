alter table flow_runs drop constraint flow_runs_run_mode_check;

alter table flow_runs
    add constraint flow_runs_run_mode_check
    check (
        run_mode in (
            'debug_node_preview',
            'debug_flow_run',
            'published_api_run',
            'workflow_http_run',
            'workflow_schedule_run'
        )
    );
