update applications
set workflow_trigger_type = 'extension'
where application_type = 'workflow'
  and workflow_trigger_type = 'manual';

alter table applications
    drop constraint applications_workflow_trigger_type_check;

alter table applications
    add constraint applications_workflow_trigger_type_check
    check (
        (
            application_type = 'workflow'
            and workflow_trigger_type in ('extension', 'schedule')
        )
        or (
            application_type <> 'workflow'
            and workflow_trigger_type is null
        )
    );
