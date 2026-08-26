alter table frontend_block_catalog
    drop constraint frontend_block_catalog_runtime_check;

-- Historical `iframe` rows remain non-executable metadata. New catalog rows
-- must use one of the two runtime kinds compiled by the Extension Bus.
alter table frontend_block_catalog
    add constraint frontend_block_catalog_runtime_check
    check (runtime in ('native_react', 'isolated_iframe')) not valid;
