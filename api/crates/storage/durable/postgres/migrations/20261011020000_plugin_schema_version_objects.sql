-- Physical ownership is shared, while each admitted plugin version retains its declared objects.
create table plugin_schema_version_objects (
    owner_id text not null,
    owner_version text not null,
    ownership_key text not null references plugin_schema_ownership(ownership_key),
    primary key (owner_id, owner_version, ownership_key)
);

-- Only current active ownership supplies evidence of version membership. Historical receipts
-- contain no object list, so older manifests must be reconciled rather than guessed here.
insert into plugin_schema_version_objects (owner_id, owner_version, ownership_key)
select owner_id, owner_version, ownership_key
from plugin_schema_ownership
where active;
