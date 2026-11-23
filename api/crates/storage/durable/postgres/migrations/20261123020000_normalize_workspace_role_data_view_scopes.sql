update role_data_policies rdp
set default_view_scope = 'scope_all', updated_at = now()
from roles r
where r.id = rdp.role_id
  and r.scope_kind = 'workspace'
  and rdp.default_view_scope = 'system_all';

update role_data_model_policies rdmp
set view_scope_override = 'scope_all', updated_at = now()
from roles r
where r.id = rdmp.role_id
  and r.scope_kind = 'workspace'
  and rdmp.view_scope_override = 'system_all';
