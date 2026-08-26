alter table application_run_trace_projection_statuses
    add column if not exists id uuid;

alter table application_run_trace_projection_statuses
    add column if not exists scope_id uuid;

alter table application_run_trace_projection_statuses
    add column if not exists created_by uuid;

alter table application_run_trace_projection_statuses
    add column if not exists updated_by uuid;

update application_run_trace_projection_statuses statuses
   set scope_id = flow_runs.scope_id
  from flow_runs
 where statuses.flow_run_id = flow_runs.id
   and statuses.scope_id is null;

with generated as (
    select
        flow_run_id,
        projection_version,
        md5(
            '1flowbase.application_run_trace_projection_status:'
            || flow_run_id::text
            || ':'
            || projection_version::text
        ) as generated_id
    from application_run_trace_projection_statuses
    where id is null
)
update application_run_trace_projection_statuses statuses
   set id = (
       substr(generated.generated_id, 1, 8)
       || '-'
       || substr(generated.generated_id, 9, 4)
       || '-'
       || substr(generated.generated_id, 13, 4)
       || '-'
       || substr(generated.generated_id, 17, 4)
       || '-'
       || substr(generated.generated_id, 21, 12)
   )::uuid
  from generated
 where statuses.flow_run_id = generated.flow_run_id
   and statuses.projection_version = generated.projection_version
   and statuses.id is null;

alter table application_run_trace_nodes
    add column if not exists id uuid;

alter table application_run_trace_nodes
    add column if not exists scope_id uuid;

alter table application_run_trace_nodes
    add column if not exists created_by uuid;

alter table application_run_trace_nodes
    add column if not exists updated_by uuid;

update application_run_trace_nodes nodes
   set scope_id = flow_runs.scope_id
  from flow_runs
 where nodes.flow_run_id = flow_runs.id
   and nodes.scope_id is null;

with generated as (
    select
        trace_node_id,
        md5('1flowbase.application_run_trace_node:' || trace_node_id::text) as generated_id
    from application_run_trace_nodes
    where id is null
)
update application_run_trace_nodes nodes
   set id = (
       substr(generated.generated_id, 1, 8)
       || '-'
       || substr(generated.generated_id, 9, 4)
       || '-'
       || substr(generated.generated_id, 13, 4)
       || '-'
       || substr(generated.generated_id, 17, 4)
       || '-'
       || substr(generated.generated_id, 21, 12)
   )::uuid
  from generated
 where nodes.trace_node_id = generated.trace_node_id
   and nodes.id is null;

alter table application_run_trace_node_contents
    add column if not exists id uuid;

alter table application_run_trace_node_contents
    add column if not exists flow_run_id uuid;

alter table application_run_trace_node_contents
    add column if not exists scope_id uuid;

alter table application_run_trace_node_contents
    add column if not exists created_by uuid;

alter table application_run_trace_node_contents
    add column if not exists updated_by uuid;

update application_run_trace_node_contents contents
   set flow_run_id = nodes.flow_run_id,
       scope_id = nodes.scope_id
  from application_run_trace_nodes nodes
 where contents.trace_node_id = nodes.trace_node_id
   and (contents.flow_run_id is null or contents.scope_id is null);

with generated as (
    select
        trace_node_id,
        md5('1flowbase.application_run_trace_node_content:' || trace_node_id::text) as generated_id
    from application_run_trace_node_contents
    where id is null
)
update application_run_trace_node_contents contents
   set id = (
       substr(generated.generated_id, 1, 8)
       || '-'
       || substr(generated.generated_id, 9, 4)
       || '-'
       || substr(generated.generated_id, 13, 4)
       || '-'
       || substr(generated.generated_id, 17, 4)
       || '-'
       || substr(generated.generated_id, 21, 12)
   )::uuid
  from generated
 where contents.trace_node_id = generated.trace_node_id
   and contents.id is null;
