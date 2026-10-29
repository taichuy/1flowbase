-- Built-in proxies own their transport and have no runtime-extension family.
-- Existing HTTP providers and all extension-backed providers retain their identities.
alter table network_egress_providers
    drop constraint network_egress_providers_extension_family_check;

alter table network_egress_providers
    add constraint network_egress_providers_extension_family_check check (
        (
            provider_code in ('builtin_static_http', 'builtin_static_socks5')
            and extension_category is null
            and extension_organization is null
            and extension_artifact_id is null
        )
        or
        (
            provider_code not in ('builtin_static_http', 'builtin_static_socks5')
            and extension_category = 'runtime-extensions'
            and extension_organization is not null
            and extension_organization <> ''
            and extension_artifact_id is not null
            and extension_artifact_id <> ''
        )
    );
