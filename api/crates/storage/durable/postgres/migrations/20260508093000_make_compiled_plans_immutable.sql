alter table flow_compiled_plans
    add column document_hash text not null default '';

alter table flow_compiled_plans
    drop constraint if exists flow_compiled_plans_flow_draft_id_key;

create index flow_compiled_plans_draft_document_idx
    on flow_compiled_plans (
        flow_draft_id,
        document_hash,
        created_at desc,
        id desc
    );
