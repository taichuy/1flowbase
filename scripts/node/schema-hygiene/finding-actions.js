

function actionForRule(rule) {
  if (rule.startsWith('exemption-')) {
    return 'Update scripts/node/schema-hygiene/config.json with a specific reason and concrete skip entries.';
  }
  if (rule.startsWith('dynamic-model-')) {
    return 'Update the dynamic model table template to include scope_id, timestamps, and a (scope_id, created_at, id) index.';
  }
  if (rule.startsWith('managed-table-')) {
    return 'Add the missing physical schema property or add a concrete, reasoned exemption for a bounded special table.';
  }
  if (rule.startsWith('flow-run-owned-table-')) {
    return 'Verify the non-null flow_runs owner reference, durable primary key, and flow_run_id-first access path.';
  }
  if (rule.startsWith('retired-archive-table-')) {
    return 'Verify the retired archive retains its source table name and original JSON record.';
  }
  if (rule.startsWith('postgres-migration-')) {
    return 'Give every PostgreSQL migration SQL file a unique leading version number before running sqlx migrations.';
  }
  if (rule.startsWith('unsupported-')) {
    return 'Extend schema-hygiene SQL parsing for this DDL shape or use live DB introspection for this case.';
  }
  return 'Inspect the schema hygiene rule and fix the table or config entry.';
}

module.exports = { actionForRule };
