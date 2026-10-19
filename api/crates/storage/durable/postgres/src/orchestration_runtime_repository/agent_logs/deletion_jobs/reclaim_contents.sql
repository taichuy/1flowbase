delete from runtime_canonical_contents c
where c.application_id=$1 and c.id=any($2)
  and not exists(select 1 from runtime_events e where e.observation_body_content_id=c.id)
  and not exists(select 1 from client_trajectory_sections s where s.content_id=c.id)
  and not exists(select 1 from runtime_context_projections p where p.actual_content_id=c.id)
  and not exists(select 1 from flow_run_recovery_history h where h.recovery_content_id=c.id)
  and not exists(select 1 from runtime_legacy_shadow_rows s where s.canonical_content_id=c.id)
