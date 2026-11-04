-- Source is reusable mounting code; an explicit destination determines management ownership.
alter table frontend_block_catalog
    add column ui_registration text not null default 'code_template'
    check (ui_registration in ('code_template', 'component'));
