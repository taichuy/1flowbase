-- Preserve configured values when legacy observation operations receive stable IDs.
-- If a role already has the new ID, its explicit policy takes precedence.
delete from role_console_operation_policies old_policy
using (values
    ('reveal_host_infrastructure_cache_entry', 'host_infrastructure.cache.reveal'),
    ('clear_host_infrastructure_cache_entry', 'host_infrastructure.cache.entry.clear'),
    ('clear_host_infrastructure_cache_domain', 'host_infrastructure.cache.domain.clear'),
    ('reveal_host_infrastructure_memory_entry', 'host_infrastructure.memory.reveal')
) as renamed(old_id, new_id)
where old_policy.operation_id = renamed.old_id
  and exists (
      select 1 from role_console_operation_policies current_policy
      where current_policy.role_id = old_policy.role_id
        and current_policy.operation_id = renamed.new_id
  );

update role_console_operation_policies policy
set operation_id = renamed.new_id, updated_at = now()
from (values
    ('reveal_host_infrastructure_cache_entry', 'host_infrastructure.cache.reveal'),
    ('clear_host_infrastructure_cache_entry', 'host_infrastructure.cache.entry.clear'),
    ('clear_host_infrastructure_cache_domain', 'host_infrastructure.cache.domain.clear'),
    ('reveal_host_infrastructure_memory_entry', 'host_infrastructure.memory.reveal')
) as renamed(old_id, new_id)
where policy.operation_id = renamed.old_id;

-- The operation-binding endpoint was removed; no current operation grants it.
delete from role_console_operation_policies
where operation_id = 'get_application_operation_bindings';
