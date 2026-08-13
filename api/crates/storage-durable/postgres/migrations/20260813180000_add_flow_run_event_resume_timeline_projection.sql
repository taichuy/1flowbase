alter table flow_run_events
    add column resume_timeline_description text,
    add column resume_timeline_description_projected boolean not null default false,
    add constraint flow_run_events_resume_timeline_projection_check check (
        resume_timeline_description_projected or resume_timeline_description is null
    );

comment on column flow_run_events.resume_timeline_description is
    'Exact resume_request_id or callback_task_id selected from this event payload for Timeline display';

comment on column flow_run_events.resume_timeline_description_projected is
    'True when resume_timeline_description was projected at event persistence, including exact absence';
