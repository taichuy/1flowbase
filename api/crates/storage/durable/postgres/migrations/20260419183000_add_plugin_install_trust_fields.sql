update plugin_installations
set source_kind = 'uploaded'
where source_kind = 'downloaded_or_uploaded';

alter table plugin_installations
    add column trust_level text not null default 'checksum_only',
    add column signature_algorithm text,
    add column signing_key_id text;

update plugin_installations
set trust_level = case
    when signature_status = 'verified' then 'verified_official'
    when source_kind in ('official_registry', 'mirror_registry', 'uploaded') then 'checksum_only'
    else 'unverified'
end;

alter table plugin_installations
    add constraint plugin_installations_source_kind_check
        check (source_kind in ('official_registry', 'mirror_registry', 'uploaded'));

alter table plugin_installations
    add constraint plugin_installations_trust_level_check
        check (trust_level in ('verified_official', 'checksum_only', 'unverified'));
