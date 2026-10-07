-- Source-neutral tasks, messages and client occurrences; no synthetic execution.
alter table applications drop constraint applications_application_type_check;
alter table applications add constraint applications_application_type_check check(application_type in ('agent_flow','workflow','agent_logs'));
alter table application_run_log_tasks drop constraint application_run_log_tasks_id_fkey;
alter table application_run_log_tasks drop constraint application_run_log_tasks_member_run_ids_check;
alter table application_run_log_tasks drop constraint application_run_log_tasks_parent_task_run_id_fkey;
-- Native parent references used run identity; canonicalize to the existing task
-- anchor before giving source-neutral parent links a task owner.
update application_run_log_tasks child set parent_task_run_id=(
 select parent.id from application_run_log_summaries source
 join application_run_log_tasks parent on parent.id=coalesce(source.log_task_run_id,source.flow_run_id)
 where source.flow_run_id=child.parent_task_run_id and source.application_id=child.application_id and parent.id<>child.id
) where child.parent_task_run_id is not null;
alter table application_run_log_tasks add constraint application_run_log_tasks_parent_task_run_id_fkey foreign key(parent_task_run_id) references application_run_log_tasks(id) on delete set null;
alter table application_run_log_tasks
 add column source_kind text not null default 'native' check(source_kind in ('native','imported')),
 add column source_id text, add column source_client text, add column source_session_id text, add column source_task_id text, add column parent_source_task_id text,
 add column native_run_id uuid references flow_runs(id) on delete cascade, add column cost_breakdown jsonb;
update application_run_log_tasks set native_run_id=id;
-- Preserve the original summary->task cascade through the nullable Native
-- association; imported tasks have neither a summary nor a flow owner.
alter table application_run_log_tasks add constraint application_run_log_tasks_native_summary_fkey
 foreign key(native_run_id) references application_run_log_summaries(flow_run_id) on delete cascade;
-- Every subsequent native projection retains its real summary and run owners.
create function bind_application_log_native_task_owner() returns trigger language plpgsql as $$
begin
 if new.source_kind='native' then
  new.native_run_id:=new.id;
  if new.parent_task_run_id is not null then
   select parent.id into new.parent_task_run_id from application_run_log_summaries source
   join application_run_log_tasks parent on parent.id=coalesce(source.log_task_run_id,source.flow_run_id)
   where source.flow_run_id=new.parent_task_run_id and source.application_id=new.application_id and parent.id<>new.id;
  end if;
 end if;
 return new;
end $$;
create trigger application_log_native_task_owner before insert or update on application_run_log_tasks
 for each row execute function bind_application_log_native_task_owner();

create unique index application_run_log_tasks_source_identity on application_run_log_tasks(application_id,source_id,source_session_id,source_task_id) where source_kind='imported';
alter table application_run_conversation_message_items alter column flow_run_id drop not null;
alter table application_run_conversation_message_items add column record_id uuid references application_run_log_tasks(id) on delete cascade;
create index application_run_message_record_sequence on application_run_conversation_message_items(application_id,record_id,display_sequence);
alter table client_trajectory_captures alter column event_id drop not null, alter column flow_run_id drop not null;
alter table client_trajectory_captures add column record_id uuid references application_run_log_tasks(id) on delete cascade;
alter table client_trajectory_steps alter column flow_run_id drop not null;
alter table client_trajectory_steps add column record_id uuid references application_run_log_tasks(id) on delete cascade;
alter table client_trajectory_sections alter column flow_run_id drop not null;
alter table client_trajectory_sections add column record_id uuid references application_run_log_tasks(id) on delete cascade;
create index client_trajectory_record_page on client_trajectory_steps(record_id,event_sequence,id);
create index client_trajectory_record_sections on client_trajectory_sections(record_id,step_id,section,event_sequence);
-- Receipt identity points to the canonical event body in the existing section store.
create table application_log_upload_receipts (
 application_id uuid not null references applications(id) on delete cascade,
 source_id text not null, event_id text not null, payload_hash text not null,
 record_id uuid not null references application_run_log_tasks(id) on delete cascade,
 step_id uuid not null references client_trajectory_steps(id) on delete cascade, rated_cost numeric,
 primary key(application_id,source_id,event_id)
);
-- These fields are consumed by the same read-only runtime list model.
insert into model_fields (id,data_model_id,scope_id,code,title,physical_column_name,field_kind,is_system,is_writable,is_required,api_required,is_unique,display_options,relation_options,sort_order,availability_status)
select gen_random_uuid(),m.id,m.scope_id,f.code,f.title,f.code,f.kind,true,false,false,false,false,'{}','{}',f.position,'available'
from model_definitions m cross join (values
 ('source_kind','Source kind','string',120),('source_client','Source client','string',121),
 ('source_session_id','Source session ID','string',122),('source_task_id','Source task ID','string',123),
 ('native_run_id','Native run ID','string',124),('cost_breakdown','Cost breakdown','json',125)
) f(code,title,kind,position)
where m.code='application_run_log_tasks' and m.scope_kind='system'
and not exists(select 1 from model_fields old where old.data_model_id=m.id and old.code=f.code);
