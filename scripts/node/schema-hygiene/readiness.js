const EXPANSION_SCOPE_COLUMN = 'scope_id';
const EXPANSION_TIME_COLUMN = 'created_at';
const EXPANSION_TIE_BREAKER_COLUMN = 'id';
const PLATFORM_FIELDS = [
  'id',
  'scope_id',
  'created_at',
  'updated_at',
  'created_by',
  'updated_by',
];

function findColumn(table, columnName) {
  return table.columns.find((column) => column.name === columnName);
}

function hasColumn(table, columnName) {
  return table.columns.some((column) => column.name === columnName);
}

function hasScopeColumn(table) {
  return hasColumn(table, EXPANSION_SCOPE_COLUMN);
}

function hasScopeTimeIndex(table) {
  return table.indexes.some((index) => {
    const [scopeColumn, timeColumn, tieBreakerColumn] = index.columns;
    return scopeColumn === EXPANSION_SCOPE_COLUMN
      && timeColumn === EXPANSION_TIME_COLUMN
      && tieBreakerColumn === EXPANSION_TIE_BREAKER_COLUMN;
  });
}

function profileForTable(table, config) {
  if (config.tableProfiles[table.name]) {
    return config.tableProfiles[table.name];
  }
  if (config.registeredSystemTables.has(table.name)) {
    return 'registered_system_table';
  }
  if (config.dynamicModelTablePatterns.some((pattern) => pattern.test(table.name))) {
    return 'dynamic_model_table';
  }
  return 'managed_table';
}

function hasFlowRunOwnerReference(table) {
  const owner = findColumn(table, 'flow_run_id');
  return Boolean(owner && !owner.nullable && table.foreignKeys.some((foreignKey) => (
    foreignKey.columns.length === 1
      && foreignKey.columns[0] === 'flow_run_id'
      && foreignKey.references.table === 'flow_runs'
      && foreignKey.references.columns.length === 1
      && foreignKey.references.columns[0] === 'id'
  )));
}

function hasFlowRunOwnerIndex(table) {
  return table.primaryKey?.columns[0] === 'flow_run_id'
    || table.indexes.some((index) => index.columns[0] === 'flow_run_id');
}

function sameColumns(actual = [], expected = []) {
  return actual.length === expected.length && actual.every((column, index) => column === expected[index]);
}

function normalizedCheck(definition) {
  return definition.replace(/^constraint\s+("[^"]+"|[a-zA-Z_][a-zA-Z0-9_$]*)\s+/iu, '')
    .replace(/^check\s*/iu, '').replace(/\s+/gu, '');
}

function physicalContractFindings(table, contract) {
  if (!contract || !contract.primaryKey?.length || !Object.keys(contract.columns || {}).length
    || !contract.foreignKeys?.length || !contract.ownershipSource) {
    return [{ rule: 'physical-contract-declaration', column: null,
      message: 'physical_contract_table requires columns, exact primary key, owner foreign keys, and ownership source' }];
  }
  const findings = [];
  const report = (rule, column, message) => findings.push({ rule: `physical-contract-${rule}`, column, message });
  for (const [name, expected] of Object.entries(contract.columns)) {
    const actual = findColumn(table, name);
    if (!actual || actual.type !== expected.type || actual.nullable !== expected.nullable) {
      report('column', name, `physical contract requires ${name} ${expected.type} ${expected.nullable ? 'nullable' : 'not null'}`);
    }
  }
  if (!sameColumns(table.primaryKey?.columns, contract.primaryKey)) {
    report('primary-key', null, `physical contract requires primary key (${contract.primaryKey.join(', ')})`);
  }
  for (const expected of contract.foreignKeys) {
    if (!table.foreignKeys.some((actual) => sameColumns(actual.columns, expected.columns)
      && actual.references.table === expected.table && sameColumns(actual.references.columns, expected.references)
      && actual.onDelete === expected.onDelete)) {
      report('owner-foreign-key', expected.columns.join(','),
        `physical contract requires (${expected.columns.join(', ')}) -> ${expected.table}(${expected.references.join(', ')}) on delete ${expected.onDelete}`);
    }
  }
  for (const expected of contract.uniqueConstraints || []) {
    if (!table.uniqueConstraints.some((actual) => sameColumns(actual.columns, expected) && !actual.predicate)) {
      report('unique', null, `physical contract requires unique (${expected.join(', ')})`);
    }
  }
  const checks = table.checks.map((check) => normalizedCheck(check.definition));
  for (const expected of contract.checks || []) {
    if (!checks.includes(normalizedCheck(expected))) {
      report('check', null, `physical contract requires CHECK ${expected}`);
    }
  }
  for (const expected of contract.indexPrefixes || []) {
    if (!table.indexes.some((index) => sameColumns(index.columns.slice(0, expected.length), expected))
      && ![table.primaryKey, ...table.uniqueConstraints].some((key) => key && sameColumns(key.columns.slice(0, expected.length), expected))) {
      report('owner-index', expected.join(','), `physical contract requires lookup index led by (${expected.join(', ')})`);
    }
  }
  return findings;
}

function profileFindingsForTable(table, profile, config = {}) {
  if (profile === 'physical_contract_table') {
    return physicalContractFindings(table, config.physicalTableContracts?.[table.name]);
  }
  if (profile === 'flow_run_owned_table') {
    const findings = [];
    if (!hasFlowRunOwnerReference(table)) {
      findings.push({
        rule: 'flow-run-owned-table-owner-column',
        column: 'flow_run_id',
        message: 'flow_run_owned_table requires a non-null flow_run_id foreign key to flow_runs(id)',
      });
    }
    if (!table.primaryKey?.columns.length) {
      findings.push({
        rule: 'flow-run-owned-table-primary-key',
        column: null,
        message: 'flow_run_owned_table requires a primary key for durable row identity',
      });
    }
    if (!hasFlowRunOwnerIndex(table)) {
      findings.push({
        rule: 'flow-run-owned-table-owner-index',
        column: 'flow_run_id',
        message: 'flow_run_owned_table requires a primary key or index led by flow_run_id',
      });
    }
    return findings;
  }

  if (profile === 'retired_archive_table') {
    return [
      ['source_table', 'text'],
      ['record', 'jsonb'],
    ].flatMap(([columnName, type]) => {
      const column = findColumn(table, columnName);
      if (column && !column.nullable && column.type === type) {
        return [];
      }
      return [{
        rule: 'retired-archive-table-record-shape',
        column: columnName,
        message: `retired_archive_table requires a non-null ${columnName} ${type} column`,
      }];
    });
  }

  return [];
}

function columnReadiness(table, columnName) {
  const column = findColumn(table, columnName);
  return {
    present: Boolean(column),
    nullable: column ? column.nullable : null,
    default: column ? column.default : null,
    type: column ? column.type : null,
  };
}

function pushAction(actions, action) {
  if (!actions.includes(action)) {
    actions.push(action);
  }
}

function categoryForTable({ profile, appendOnly, systemScope, declaration }) {
  if (declaration.category) {
    return declaration.category;
  }
  if (profile === 'dynamic_model_table') {
    return 'dynamic_runtime_table';
  }
  if (profile === 'registered_system_table') {
    return 'registered_system_table';
  }
  if (systemScope) {
    return 'system_global';
  }
  if (appendOnly) {
    return 'join_or_child';
  }
  return 'unknown_needs_review';
}

function declaredSource(value) {
  if (typeof value === 'string' && value.trim().length > 0) {
    return value.trim();
  }
  return null;
}

function scopeGenerationSourceForTable({ table, systemScope, declaration }) {
  const declared = declaredSource(declaration.scopeGenerationSource || declaration.scopeSource);
  if (declared) {
    return {
      status: 'declared',
      source: declared,
    };
  }

  if (systemScope) {
    return {
      status: 'declared',
      source: 'SYSTEM_SCOPE_ID',
    };
  }

  if (hasColumn(table, 'workspace_id')) {
    return {
      status: 'inferred',
      source: 'workspace_id',
    };
  }

  return {
    status: 'needs_owner_review',
    source: null,
  };
}

function backfillSourceForTable({ table, declaration, scopeGenerationSource }) {
  const declared = declaredSource(declaration.backfillSource);
  if (declared) {
    return declared;
  }
  if (!hasScopeColumn(table) && scopeGenerationSource.status === 'inferred') {
    return scopeGenerationSource.source;
  }
  return null;
}

function generationStatusForColumn({ columnName, table, declaration }) {
  const key = `${columnName}Generation`;
  const source = declaredSource(declaration[key]);
  if (source) {
    return {
      status: 'declared',
      source,
    };
  }
  const column = findColumn(table, columnName);
  if (columnName === 'id') {
    return {
      status: column ? 'declare_generation_rule' : 'missing',
      source: null,
    };
  }
  if ((columnName === 'created_at' || columnName === 'updated_at') && column && column.default) {
    return {
      status: 'schema_default',
      source: 'default now()',
    };
  }
  if (column) {
    return {
      status: 'declare_generation_rule',
      source: null,
    };
  }
  return {
    status: 'missing',
    source: null,
  };
}

function recommendedActionsForTable({
  table,
  appendOnly,
  systemScope,
  hasReadinessIndex,
  scopeGenerationSource,
  backfillSource,
  declaration,
}) {
  const actions = [];
  if (!hasColumn(table, 'id')) {
    pushAction(actions, 'add_id');
    pushAction(actions, 'needs_owner_review');
    return actions;
  }
  if (!hasColumn(table, 'created_at')) {
    pushAction(actions, 'add_created_at');
    pushAction(actions, 'needs_owner_review');
    return actions;
  }
  if (!systemScope && !hasScopeColumn(table) && !backfillSource) {
    pushAction(actions, 'needs_owner_review');
    return actions;
  }
  if (!appendOnly && !hasColumn(table, 'updated_at')) {
    pushAction(actions, 'add_updated_at');
  }
  if (systemScope && !hasScopeColumn(table)) {
    pushAction(actions, 'mark_system_scope');
  }
  if (!systemScope && !hasScopeColumn(table)) {
    pushAction(actions, 'add_scope_id');
    pushAction(actions, 'backfill_scope_id');
  }
  if (!systemScope && (hasScopeColumn(table) || backfillSource) && !hasReadinessIndex) {
    pushAction(actions, 'add_scope_time_index');
  }

  const writePathSource = declaredSource(declaration.writePathSource);
  const hasMissingAuditColumns = !hasColumn(table, 'created_by') || !hasColumn(table, 'updated_by');
  const generationNeedsDeclaration = !writePathSource
    || generationStatusForColumn({ columnName: 'id', table, declaration }).status === 'declare_generation_rule'
    || generationStatusForColumn({ columnName: 'created_by', table, declaration }).status === 'declare_generation_rule'
    || generationStatusForColumn({ columnName: 'updated_by', table, declaration }).status === 'declare_generation_rule'
    || hasMissingAuditColumns;
  if (generationNeedsDeclaration && !actions.includes('needs_owner_review')) {
    pushAction(actions, 'declare_generation_rule');
  }

  if (actions.length === 0) {
    pushAction(actions, 'no_action');
  }
  return actions;
}

function platformReadinessForTable({ table, profile, config, tableFindings }) {
  const appendOnly = config.appendOnlyTables.has(table.name);
  const systemScope = config.systemScopeTables.has(table.name);
  const declaration = {
    ...config.defaultTableReadiness,
    ...(config.tableReadiness[table.name] || {}),
  };
  const needsOwnerReviewReason = config.needsOwnerReviewTables[table.name] || null;
  const fields = Object.fromEntries(
    [...PLATFORM_FIELDS, 'workspace_id'].map((columnName) => [
      columnName,
      {
        ...columnReadiness(table, columnName),
        generation: PLATFORM_FIELDS.includes(columnName)
          ? generationStatusForColumn({ columnName, table, declaration })
          : null,
      },
    ])
  );
  const hasReadinessIndex = hasScopeTimeIndex(table);
  const scopeGenerationSource = scopeGenerationSourceForTable({
    table,
    systemScope,
    declaration,
  });
  const backfillSource = backfillSourceForTable({
    table,
    declaration,
    scopeGenerationSource,
  });
  if (needsOwnerReviewReason) {
    return {
      category: 'unknown_needs_review',
      timeKey: EXPANSION_TIME_COLUMN,
      tieBreaker: EXPANSION_TIE_BREAKER_COLUMN,
      fields,
      missingFields: PLATFORM_FIELDS.filter((columnName) => !hasColumn(table, columnName)),
      requiredScopeId: !systemScope,
      routingKeyStatus: hasScopeColumn(table) ? 'present' : 'missing',
      scopeGenerationSource,
      backfillSource: null,
      writePathSource: declaredSource(declaration.writePathSource),
      hasScopeTimeIdIndex: hasReadinessIndex,
      recommendedActions: ['needs_owner_review'],
      severity: 'warning',
      reason: needsOwnerReviewReason,
    };
  }
  if (profile === 'physical_contract_table') {
    const invalid = tableFindings.some((item) => item.severity === 'error');
    const contract = config.physicalTableContracts[table.name];
    return {
      category: profile,
      timeKey: null,
      tieBreaker: contract?.primaryKey || null,
      fields,
      missingFields: tableFindings.filter((item) => item.severity === 'error').map((item) => item.column || item.rule),
      requiredScopeId: Boolean(contract?.columns?.scope_id),
      routingKeyStatus: invalid ? 'needs_owner_review' : 'owner_foreign_key',
      scopeGenerationSource: { status: invalid ? 'needs_owner_review' : 'derived', source: contract?.ownershipSource || null },
      backfillSource: null,
      writePathSource: contract?.writePathSource || null,
      hasScopeTimeIdIndex: false,
      recommendedActions: [invalid ? 'review_profile_contract' : 'no_action'],
      severity: invalid ? 'error' : 'ok',
      reason: invalid ? 'physical identity or ownership contract requires repair' : 'physical columns, identity, ownership and integrity checks passed',
    };
  }
  if (profile === 'flow_run_owned_table' || profile === 'retired_archive_table') {
    const invalid = tableFindings.some((item) => item.severity === 'error');
    const flowRunOwned = profile === 'flow_run_owned_table';
    return {
      category: profile,
      timeKey: null,
      tieBreaker: null,
      fields,
      missingFields: tableFindings
        .filter((item) => item.severity === 'error')
        .map((item) => item.column || item.rule),
      requiredScopeId: false,
      routingKeyStatus: flowRunOwned ? 'flow_run_id' : 'not_applicable',
      scopeGenerationSource: flowRunOwned
        ? { status: invalid ? 'needs_owner_review' : 'derived', source: invalid ? null : 'flow_run_id -> flow_runs.id' }
        : { status: 'not_required', source: 'retired archive data is not runtime-owned' },
      backfillSource: null,
      writePathSource: declaredSource(declaration.writePathSource),
      hasScopeTimeIdIndex: false,
      hasFlowRunOwnerIndex: flowRunOwned ? hasFlowRunOwnerIndex(table) : null,
      recommendedActions: [invalid ? 'review_profile_contract' : 'no_action'],
      severity: invalid ? 'error' : 'ok',
      reason: invalid
        ? 'specialized table ownership contract requires repair'
        : flowRunOwned
          ? 'run ownership, primary key, and run lookup index checks passed'
          : 'retired archive record shape checks passed',
    };
  }

  const recommendedActions = recommendedActionsForTable({
    table,
    appendOnly,
    systemScope,
    hasReadinessIndex,
    scopeGenerationSource,
    backfillSource,
    declaration,
  });
  const severity = tableFindings.some((item) => item.severity === 'error')
    ? 'error'
    : recommendedActions.includes('no_action') ? 'ok' : 'warning';

  return {
    category: categoryForTable({ profile, appendOnly, systemScope, declaration }),
    timeKey: EXPANSION_TIME_COLUMN,
    tieBreaker: EXPANSION_TIE_BREAKER_COLUMN,
    fields,
    missingFields: PLATFORM_FIELDS.filter((columnName) => !hasColumn(table, columnName)),
    requiredScopeId: !systemScope,
    routingKeyStatus: hasScopeColumn(table) ? 'present' : 'missing',
    scopeGenerationSource,
    backfillSource,
    writePathSource: declaredSource(declaration.writePathSource),
    hasScopeTimeIdIndex: hasReadinessIndex,
    recommendedActions,
    severity,
    reason: recommendedActions.includes('no_action')
      ? 'platform field and expansion readiness checks passed'
      : 'platform field or generation source action required',
  };
}

module.exports = {
  EXPANSION_SCOPE_COLUMN,
  hasColumn,
  hasScopeColumn,
  hasScopeTimeIndex,
  hasFlowRunOwnerIndex,
  platformReadinessForTable,
  profileFindingsForTable,
  profileForTable,
};
