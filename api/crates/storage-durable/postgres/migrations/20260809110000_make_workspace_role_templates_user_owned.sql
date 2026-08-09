update roles
set is_builtin = false,
    is_editable = true,
    system_kind = null,
    updated_at = now()
where scope_kind = 'workspace'
  and is_builtin = true
  and code in ('admin', 'member');
