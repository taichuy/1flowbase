-- Attribution is a historical identifier, not ownership of a live user row.
-- Keep the UUID (and existing NOT NULL contracts) when the user is removed.
-- Scope catalog changes to this migration's schema, including isolated tests.
do $$
declare
    reference record;
begin
    for reference in
        select namespace.nspname, relation.relname, constraint_record.conname
        from pg_constraint constraint_record
        join pg_class relation on relation.oid = constraint_record.conrelid
        join pg_namespace namespace on namespace.oid = relation.relnamespace
        join pg_attribute attribute
          on attribute.attrelid = relation.oid
         and attribute.attnum = constraint_record.conkey[1]
        where constraint_record.contype = 'f'
          and constraint_record.confrelid = 'users'::regclass
          and namespace.nspname = current_schema()
          and cardinality(constraint_record.conkey) = 1
          and constraint_record.confdeltype in ('a', 'r')
          and attribute.attname in (
              'created_by', 'updated_by', 'actor_user_id', 'imported_by',
              'assigned_by', 'granted_by', 'revoked_by'
          )
    loop
        execute format('alter table %I.%I drop constraint %I',
            reference.nspname, reference.relname, reference.conname);
    end loop;
end;
$$;

-- Conversations and financial history must outlive their user. Keeping the
-- account also prevents ledger/session account_id RESTRICT constraints from
-- blocking the user's deletion through the former CASCADE chain.
alter table assistant_conversations
    drop constraint assistant_conversations_created_by_fkey;
alter table user_credit_accounts
    drop constraint user_credit_accounts_user_id_fkey;

-- User credentials still cascade away; their conversation history must not.
alter table application_public_conversations
    drop constraint application_public_conversations_api_key_id_fkey;
