'use strict';
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const literal = value => "'" + value.replaceAll("'", "''") + "'";

// Runs only in the driver's proof-owned database. The real migration acts on a
// temporary copy of the installed schema; no persistent installation is changed.
function verifyCollectorMigration({ root, container, manifest }) {
  const migrations = path.join(root, 'api/crates/storage/durable/postgres/migrations');
  const old = fs.readFileSync(path.join(migrations, '20260803010000_unify_extension_installation_lifecycle.sql'), 'utf8');
  const match = old.match(/constraint extension_installations_plugin_contract_check check \([\s\S]*?\n    \),/);
  assert.ok(match, 'original installation constraint must exist');
  const originalConstraint = match[0].slice(0, -1);
  const migration = fs.readFileSync(path.join(migrations, '20261008120000_client_collector_installation_contract.sql'), 'utf8');
  const receipt = JSON.stringify({ distribution_kind: 'client_collector', client_collector: manifest });
  const columns = 'id,category,organization,artifact_id,artifact_version,display_name,source_kind,trust_level,signature_status,created_by,receipt,plugin_id,contract_version,protocol,verification_status,desired_state,application_action';
  const shared = "'official_repository','checksum_only','missing','00000000-0000-0000-0000-000000000001'";
  const clientInsert = `insert into extension_installations (${columns}) values ('00000000-0000-0000-0000-000000000003','runtime-extensions',${literal(manifest.organization)},${literal(manifest.artifact_id)},${literal(manifest.version)},'Client',${shared},${literal(receipt)}::jsonb,null,null,null,'pending',null,'none');`;
  function rejection(statement) {
    return `do $$ declare rejected_constraint text; begin
      begin
        ${statement}
        raise exception 'expected installation contract rejection';
      exception when check_violation then
        get stacked diagnostics rejected_constraint = CONSTRAINT_NAME;
        if rejected_constraint <> 'extension_installations_plugin_contract_check' then
          raise exception 'wrong constraint rejected fixture: %', rejected_constraint;
        end if;
      end;
    end $$;`;
  }
  const invalidUpdates = [
    "receipt = receipt - 'distribution_kind'",
    "receipt = jsonb_set(receipt,'{client_collector,execution_target}','\"server\"')",
    "receipt = jsonb_set(receipt,'{client_collector,protocol_version}','\"unsupported/v1\"')",
    "receipt = jsonb_set(receipt,'{client_collector,schema_version}','\"unsupported/v1\"')",
    "receipt = jsonb_set(receipt,'{client_collector,distribution_kind}','\"runtime_plugin\"')",
    "receipt = jsonb_set(receipt,'{client_collector,organization}','\"other\"')",
    "receipt = jsonb_set(receipt,'{client_collector,version}','\"9.9.9\"')",
    "receipt = receipt - 'client_collector'",
    "plugin_id = 'pretend-server', contract_version = 'v1', protocol = 'stdio', desired_state = 'active_requested'",
    "category = 'host-extensions'",
    "application_action = 'configure_model_provider'",
  ];
  const sql = `begin;
    create temporary table extension_installations (like public.extension_installations including all);
    alter table extension_installations drop constraint extension_installations_plugin_contract_check, add ${originalConstraint};
    insert into extension_installations (${columns}) values
      ('00000000-0000-0000-0000-000000000001','runtime-extensions','fixture','server','1.0.0','Server',${shared},'{}','fixture/server','v1','stdio','valid','active_requested','none'),
      ('00000000-0000-0000-0000-000000000002','mcp','fixture','bundle','1.0.0','MCP',${shared},'{}',null,null,null,null,null,'import_mcp');
    create temporary table before_upgrade as select id,to_jsonb(installed_row) as value from extension_installations installed_row;
    ${rejection(clientInsert)}
    ${migration}
    do $$ begin
      if exists(select 1 from before_upgrade prior_snapshot left join extension_installations current_record using(id) where prior_snapshot.value is distinct from to_jsonb(current_record)) then
        raise exception 'forward migration changed an existing installation';
      end if;
    end $$;
    ${clientInsert}
    ${invalidUpdates.map(set => rejection(`update extension_installations set ${set} where id = '00000000-0000-0000-0000-000000000003';`)).join('\n')}
    ${rejection("update extension_installations set plugin_id = null where id = '00000000-0000-0000-0000-000000000001';")}
    select count(*) from extension_installations;
    rollback;`;
  const output = execFileSync('docker', ['exec', '-i', container, 'psql', '-X', '-v', 'ON_ERROR_STOP=1', '-U', 'postgres', '-d', 'collector_proof', '-At'],
    { input: sql, encoding: 'utf8', maxBuffer: 1024 * 1024, stdio: ['pipe', 'pipe', 'pipe'] });
  assert.ok(output.split('\n').includes('3'), 'two historical rows and one client row retained');
  return { name: 'forward installation constraint migration', status: 'pass', existing_rows_unchanged: 2, rejection_cases: invalidUpdates.length + 2, scope: 'proof DB temporary table, rollback' };
}
module.exports = { verifyCollectorMigration };
