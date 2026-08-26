alter table ui_component_records
    add column catalog_updated_at timestamptz,
    add column source_locator text,
    add column source_checksum text;

alter table ui_component_records
    add constraint ui_component_records_source_checksum_check check (
        source_checksum is null
        or source_checksum ~ '^sha256:[a-f0-9]{64}$'
    );
