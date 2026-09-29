const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { collectSchemaInventory, evaluateSchemaHygiene, loadConfig } = require('../core.js');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
const config = loadConfig(repoRoot);
const physicalNames = Object.keys(config.physicalTableContracts);
let currentInventory;

function inventoryFor(name) {
  currentInventory ||= collectSchemaInventory({ repoRoot });
  return { ...currentInventory, tables: structuredClone(currentInventory.tables.filter((table) => table.name === name)) };
}

function reportFor(name, mutate = () => {}, configured = config) {
  const inventory = inventoryFor(name);
  assert.equal(inventory.tables.length, 1, `${name} must resolve from real migration history`);
  mutate(inventory.tables[0]);
  return evaluateSchemaHygiene({ inventory, config: configured });
}

function assertRule(report, rule) {
  assert.ok(report.findings.some((finding) => finding.rule === rule && finding.severity === 'error'),
    `${rule} must reject the controlled invalid physical contract`);
}

function inventoryFromSql(sql, inspect) {
  const temporaryRepo = fs.mkdtempSync(path.join(os.tmpdir(), 'schema-physical-contract-'));
  try {
    const migrations = path.join(temporaryRepo, 'api/crates/storage/durable/postgres/migrations');
    fs.mkdirSync(migrations, { recursive: true });
    fs.writeFileSync(path.join(migrations, '20260101000000_fixture.sql'), sql);
    inspect(collectSchemaInventory({ repoRoot: temporaryRepo }));
  } finally {
    fs.rmSync(temporaryRepo, { recursive: true, force: true });
  }
}

test('node_runs physical directory matches builtin metadata and keeps payloads in section details', () => {
  const directory = reportFor('node_runs');
  assert.equal(directory.tables[0].profile, 'registered_system_table');
  assert.deepEqual(directory.findings, []);
  const source = fs.readFileSync(path.join(repoRoot, 'api/crates/domain/src/builtin_data_model.rs'), 'utf8');
  const fields = /const NODE_RUNS_FIELDS: &\[&str\] = &\[([\s\S]*?)\];/u.exec(source);
  assert.ok(fields, 'builtin metadata must declare node directory fields');
  const metadataFields = [...fields[1].matchAll(/"([a-z_]+)"/gu)].map((match) => match[1]);
  assert.deepEqual([...config.registeredSystemTableTemplates.node_runs.requiredColumns].sort(), metadataFields.sort());
  for (const field of ['input_payload', 'output_payload', 'error_payload', 'metrics_payload', 'debug_payload', 'raw_json_payloads']) {
    assert.equal(directory.tables[0].columns.some((column) => column.name === field), false, `${field} belongs outside node_runs`);
  }
  const detail = reportFor('node_run_details');
  assert.deepEqual(detail.findings, []);
  assert.deepEqual(detail.tables[0].primaryKey.columns, ['node_run_id', 'section']);
  assert.deepEqual(detail.tables[0].columns.map((column) => column.name), ['node_run_id', 'section', 'payload', 'raw_json_payloads']);
});

for (const name of physicalNames) {
  test(`${name} scans its current physical identity and owner contract`, () => {
    const report = reportFor(name);
    assert.deepEqual(report.findings, []);
    assert.equal(report.tables[0].platformReadiness.severity, 'ok');
    assert.equal(report.tables[0].profile, name === 'node_runs' ? 'registered_system_table' : 'physical_contract_table');
  });

  test(`${name} rejects missing columns and absent or wrong primary keys`, () => {
    const required = Object.keys(config.physicalTableContracts[name].columns)[0];
    assertRule(reportFor(name, (table) => { table.columns = table.columns.filter((column) => column.name !== required); }), 'physical-contract-column');
    assertRule(reportFor(name, (table) => { table.primaryKey = null; }), 'physical-contract-primary-key');
    assertRule(reportFor(name, (table) => { table.primaryKey.columns = ['wrong_identity']; }), 'physical-contract-primary-key');
  });

  test(`${name} rejects absent, wrong, or wrong-lifecycle owner foreign keys`, () => {
    for (let index = 0; index < config.physicalTableContracts[name].foreignKeys.length; index += 1) {
      const expected = config.physicalTableContracts[name].foreignKeys[index];
      const findOwner = (table) => table.foreignKeys.find((key) => key.columns.join(',') === expected.columns.join(','));
      assertRule(reportFor(name, (table) => {
        table.foreignKeys = table.foreignKeys.filter((key) => key !== findOwner(table));
      }), 'physical-contract-owner-foreign-key');
      assertRule(reportFor(name, (table) => { findOwner(table).references.table = 'wrong_owner'; }), 'physical-contract-owner-foreign-key');
      assertRule(reportFor(name, (table) => { findOwner(table).references.columns = ['wrong_identity']; }), 'physical-contract-owner-foreign-key');
      assertRule(reportFor(name, (table) => { findOwner(table).onDelete = 'restrict'; }), 'physical-contract-owner-foreign-key');
    }
  });

  test(`${name} rejects wrong column types and ownership nullability`, () => {
    const [required, shape] = Object.entries(config.physicalTableContracts[name].columns)[0];
    assertRule(reportFor(name, (table) => { table.columns.find((column) => column.name === required).type = 'text'; }), 'physical-contract-column');
    assertRule(reportFor(name, (table) => { table.columns.find((column) => column.name === required).nullable = !shape.nullable; }), 'physical-contract-column');
  });
}

for (const name of physicalNames.filter((table) => config.physicalTableContracts[table].uniqueConstraints)) {
  test(`${name} rejects lost uniqueness`, () => {
    assertRule(reportFor(name, (table) => { table.uniqueConstraints = []; }), 'physical-contract-unique');
  });
}

for (const name of physicalNames.filter((table) => config.physicalTableContracts[table].checks)) {
  test(`${name} rejects lost integrity checks`, () => {
    for (const expected of config.physicalTableContracts[name].checks) {
      // Remove one real expression at a time, including inline CHECKs.
      const normalize = (value) => value.replace(/^constraint\s+\w+\s+/iu, '').replace(/^check\s*/iu, '').replace(/\s+/gu, '');
      assertRule(reportFor(name, (table) => {
        table.checks = table.checks.filter((check) => normalize(check.definition) !== normalize(expected));
      }), 'physical-contract-check');
    }
  });
}

test('physical profiles require declarations and cannot skip their contract checks', () => {
  const missingContract = structuredClone(config);
  delete missingContract.physicalTableContracts.node_run_details;
  assertRule(reportFor('node_run_details', () => {}, missingContract), 'physical-contract-declaration');
  const skips = structuredClone(config);
  skips.exemptions.node_run_details = {
    reason: 'section payload rows belong to one node run owner',
    skip: ['physical-contract-primary-key', 'physical-contract-owner-foreign-key', 'physical-contract-column'],
  };
  const report = reportFor('node_run_details', (table) => { table.primaryKey = null; table.foreignKeys = []; }, skips);
  assertRule(report, 'physical-contract-primary-key');
  assertRule(report, 'physical-contract-owner-foreign-key');
});

test('unbound archive heads are legal while scoped snapshot owners remain mandatory', () => {
  assert.deepEqual(reportFor('client_trajectory_archive_heads').findings, []);
  const ownership = config.physicalTableContracts.runtime_native_snapshot_items;
  assert.deepEqual(ownership.foreignKeys[0].columns, ['application_id', 'scope_id']);
  assertRule(reportFor('runtime_native_snapshot_items', (table) => {
    table.columns.find((column) => column.name === 'scope_id').nullable = true;
  }), 'physical-contract-column');
  assertRule(reportFor('client_trajectory_archive_parts', (table) => {
    table.foreignKeys.find((key) => key.columns.includes('block_id')).columns = ['block_id'];
  }), 'physical-contract-owner-foreign-key');
});

test('native item reverse ownership and archive paging indexes remain scanned', () => {
  for (const name of ['runtime_native_snapshot_references', 'client_trajectory_archive_parts']) {
    assertRule(reportFor(name, (table) => { table.indexes = []; }), 'physical-contract-owner-index');
  }
});

test('ALTER DROP CHECK removes retired checks instead of accepting them as current integrity', () => {
  inventoryFromSql(`create table node_run_details (
    node_run_id uuid not null references node_runs(id) on delete cascade,
    section text not null, payload jsonb, raw_json_payloads jsonb not null,
    primary key(node_run_id,section),
    constraint details_payload check(section not in ('input_payload','output_payload','metrics_payload','debug_payload') or payload is not null)
  ); alter table node_run_details drop constraint details_payload;`, (inventory) => {
    assert.deepEqual(inventory.tables[0].checks, []);
    assertRule(evaluateSchemaHygiene({ inventory, config }), 'physical-contract-check');
  });
});

test('dropping an inline CHECK cannot satisfy a required physical integrity check', () => {
  const contractConfig = structuredClone(config);
  contractConfig.physicalTableContracts.runtime_native_snapshot_items.checks = ['(byte_size=octet_length(body))'];
  inventoryFromSql(`create table runtime_native_snapshot_items (
    id uuid primary key, scope_id uuid not null, application_id uuid not null,
    content_hash text not null, body text not null,
    byte_size bigint not null check(byte_size=octet_length(body)),
    unique(application_id,content_hash),
    foreign key(application_id,scope_id) references applications(id,scope_id) on delete cascade
  ); alter table runtime_native_snapshot_items drop constraint runtime_native_snapshot_items_byte_size_check;`, (inventory) => {
    assert.deepEqual(inventory.tables[0].checks, []);
    assertRule(evaluateSchemaHygiene({ inventory, config: contractConfig }), 'physical-contract-check');
  });
});

test('dropping generated PK and owner FK names rejects section details', () => {
  inventoryFromSql(`create table node_run_details (
    node_run_id uuid not null references node_runs(id) on delete cascade,
    section text not null, payload jsonb, raw_json_payloads jsonb not null,
    primary key(node_run_id,section),
    check(section not in ('input_payload','output_payload','metrics_payload','debug_payload') or payload is not null)
  ); alter table node_run_details drop constraint node_run_details_pkey,
     drop constraint node_run_details_node_run_id_fkey;`, (inventory) => {
    const report = evaluateSchemaHygiene({ inventory, config });
    assertRule(report, 'physical-contract-primary-key');
    assertRule(report, 'physical-contract-owner-foreign-key');
  });
});

test('ordinary managed tables still require platform fields and expansion index', () => {
  inventoryFromSql('create table ordinary_storage (body text);', (inventory) => {
    const report = evaluateSchemaHygiene({ inventory, config });
    assert.equal(report.tables[0].profile, 'managed_table');
    for (const rule of ['managed-table-id', 'managed-table-created-at', 'managed-table-updated-at-or-append-only', 'managed-table-scope-column', 'managed-table-scope-time-index']) {
      assertRule(report, rule);
    }
  });
});
