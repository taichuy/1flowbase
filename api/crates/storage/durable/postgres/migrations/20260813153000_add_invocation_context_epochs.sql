alter table runtime_context_projections
    drop constraint if exists runtime_context_projections_transition_kind_check;

alter table runtime_context_projections
    add constraint runtime_context_projections_transition_kind_check check (
        transition_kind in (
            'initial', 'append', 'callback', 'retry',
            'declared_compaction', 'observed_replacement'
        )
    );
