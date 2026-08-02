alter table extension_installations
    add column is_current boolean not null default false;

with ranked as (
    select id,
           row_number() over (
               partition by category, organization, artifact_id, node_id
               order by updated_at desc, id desc
           ) as position
    from extension_installations
    where status = 'installed'
)
update extension_installations installation
set is_current = true
from ranked
where installation.id = ranked.id and ranked.position = 1;

create unique index extension_installations_one_current_family_idx
    on extension_installations (category, organization, artifact_id, node_id)
    where is_current;

create index extension_installations_node_category_current_idx
    on extension_installations (node_id, category, is_current desc, updated_at desc);
