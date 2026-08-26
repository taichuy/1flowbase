-- Static workspace naming is already finalized in the baseline migration.
-- Keep this migration so historical replay order stays stable while root tenant
-- seeding remains idempotent for empty-database bootstrap.

insert into tenants (id, code, name, is_root, is_hidden)
values (
  '00000000-0000-0000-0000-000000000001',
  'root-tenant',
  'Root Tenant',
  true,
  true
)
on conflict (code) do update
set name = excluded.name,
    is_root = excluded.is_root,
    is_hidden = excluded.is_hidden,
    updated_at = now();
