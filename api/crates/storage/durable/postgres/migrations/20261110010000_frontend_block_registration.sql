-- Mounting contributions and reusable components/templates do not implicitly opt into page creation.
alter table frontend_block_catalog add column block_registration boolean not null default false;
-- The bundled core manifest explicitly registers its page blocks; preserve those existing entries.
update frontend_block_catalog as catalog set block_registration = true
from extension_installations as installation
where catalog.installation_id = installation.id and installation.source_kind = 'builtin';
