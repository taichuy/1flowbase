-- Empty timezone follows the scheduler host's system timezone at dispatch time.
-- Keep existing explicit timezone selections unchanged.
alter table workflow_schedule_triggers
    drop constraint if exists workflow_schedule_triggers_timezone_nonempty_chk;

comment on column workflow_schedule_triggers.timezone is
    'IANA timezone name; an empty string follows the scheduler host system timezone.';
