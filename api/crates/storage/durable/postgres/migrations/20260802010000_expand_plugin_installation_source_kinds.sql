alter table plugin_installations
    drop constraint if exists plugin_installations_source_kind_check;

alter table plugin_installations
    add constraint plugin_installations_source_kind_check
        check (source_kind in (
            'builtin',
            'official_registry',
            'mirror_registry',
            'uploaded',
            'official_repository',
            'configured_mirror',
            'configured_proxy'
        ));
