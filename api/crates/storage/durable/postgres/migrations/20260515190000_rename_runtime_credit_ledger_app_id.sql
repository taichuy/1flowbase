do $$
begin
    if exists (
        select 1
        from information_schema.columns
        where table_schema = current_schema()
          and table_name = 'runtime_credit_ledger'
          and column_name = 'app_id'
    ) and not exists (
        select 1
        from information_schema.columns
        where table_schema = current_schema()
          and table_name = 'runtime_credit_ledger'
          and column_name = 'application_id'
    ) then
        alter table runtime_credit_ledger rename column app_id to application_id;
    end if;
end $$;
