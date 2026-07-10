alter table frontend_block_catalog
    add column if not exists code_template text,
    add column if not exists code_template_version text,
    add column if not exists code_template_language text,
    add column if not exists code_modules jsonb not null default '[]'::jsonb;

alter table frontend_block_catalog
    add constraint frontend_block_catalog_code_template_pair_check
    check (
        (code_template is null and code_template_version is null and code_template_language is null)
        or (
            code_template is not null
            and code_template_version is not null
            and code_template_language is not null
            and length(trim(code_template)) > 0
            and length(code_template) <= 262144
            and length(trim(code_template_version)) > 0
            and code_template_language in ('jsx', 'tsx')
        )
    );

alter table frontend_block_catalog
    add constraint frontend_block_catalog_code_modules_array_check
    check (jsonb_typeof(code_modules) = 'array');
