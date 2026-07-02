alter table applications
    add column workflow_trigger_type text null;

update applications
set workflow_trigger_type = 'extension'
where application_type = 'workflow'
  and workflow_trigger_type is null;

alter table applications
    add constraint applications_workflow_trigger_type_check
    check (
        (
            application_type = 'workflow'
            and workflow_trigger_type in ('extension', 'schedule', 'manual')
        )
        or (
            application_type <> 'workflow'
            and workflow_trigger_type is null
        )
    );
