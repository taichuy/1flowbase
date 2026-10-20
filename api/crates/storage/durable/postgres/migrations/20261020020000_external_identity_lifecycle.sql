-- A connection selects credentials; authority owns the durable external subject.
alter table user_auth_identities
    add column authority_namespace text,
    add column state text not null default 'bound' check (state in ('bound', 'unlinked')),
    add column bound_at timestamptz not null default now(),
    add column unlinked_at timestamptz;

update user_auth_identities set authority_namespace = 'connection:' || connection_id::text,
    bound_at = created_at;

-- Only a Host-recorded installation id joined to verified installation evidence
-- can migrate managed namespaces. Provider claims cannot choose their authority.
update user_auth_identities identity
set authority_namespace = 'managed:' || installation.organization || '/' || installation.artifact_id || ':' || connection.auth_type
from extension_installations installation, authentication_connections connection
where identity.connection_id = connection.id
  and identity.metadata ->> 'installation_id' = installation.id::text
  and installation.category in ('capability-plugins', 'runtime-extensions')
  and installation.contract_version = '1flowbase.extension-bus/v1'
  and installation.verification_status = 'valid'
  and exists (select 1 from jsonb_array_elements(coalesce(installation.metadata_json -> 'authentication_providers', '[]'::jsonb)) provider where provider ->> 'auth_type' = connection.auth_type)
  and connection.auth_type <> 'password-local';

do $$
begin
    if exists (
        select 1 from user_auth_identities
        where connection_id <> '00000000-0000-0000-0000-000000000001'::uuid
        group by authority_namespace, coalesce(issuer, ''), coalesce(realm, ''), subject_type, subject_value
        having count(distinct user_id) > 1
    ) then
        raise exception 'external_identity_canonical_owner_conflict: resolve owners before migrating';
    end if;
end $$;

-- Same canonical subject and owner may have been recorded through several connections.
with duplicates as (
    select id, row_number() over (partition by authority_namespace, coalesce(issuer, ''), coalesce(realm, ''), subject_type, subject_value order by created_at, id) as rank
    from user_auth_identities where connection_id <> '00000000-0000-0000-0000-000000000001'::uuid
)
delete from user_auth_identities identity using duplicates where identity.id = duplicates.id and duplicates.rank > 1;

drop index user_auth_identities_external_subject_uidx;
create unique index user_auth_identities_external_subject_uidx on user_auth_identities
    (authority_namespace, coalesce(issuer, ''), coalesce(realm, ''), subject_type, subject_value)
    where connection_id <> '00000000-0000-0000-0000-000000000001'::uuid;

-- Local identity writers keep their existing narrow insert contract.
create function default_auth_identity_authority() returns trigger language plpgsql as $$
begin
    if new.authority_namespace is null then
        new.authority_namespace := 'connection:' || new.connection_id::text;
    end if;
    return new;
end $$;
create trigger user_auth_identity_authority_default before insert on user_auth_identities
    for each row execute function default_auth_identity_authority();
alter table user_auth_identities alter column authority_namespace set not null;
alter table user_auth_identities add constraint user_auth_identity_authority_nonempty check (authority_namespace <> '');
