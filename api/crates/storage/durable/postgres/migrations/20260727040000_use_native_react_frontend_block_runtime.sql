alter table frontend_block_catalog
    drop constraint frontend_block_catalog_runtime_check;

-- Existing iframe rows are historical metadata and are not executed by the
-- Native React runtime. NOT VALID preserves those rows while enforcing the
-- canonical runtime for every new or updated catalog entry.
alter table frontend_block_catalog
    add constraint frontend_block_catalog_runtime_check
    check (runtime in ('native_react')) not valid;
