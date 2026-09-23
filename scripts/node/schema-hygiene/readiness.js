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

function profileFindingsForTable(table, profile) {
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
