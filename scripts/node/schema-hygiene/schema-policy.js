const { ownedTableFindings } = require('./ownership.js');
const fs = require('node:fs');
const path = require('node:path');
const { EXPANSION_SCOPE_COLUMN, hasColumn, hasScopeColumn, hasScopeTimeIndex, platformReadinessForTable, profileFindingsForTable, profileForTable } = require('./readiness.js');
const { actionForRule } = require("./finding-actions.js");

const DEFAULT_CONFIG_FILE = path.join('scripts', 'node', 'schema-hygiene', 'config.json');

const DEFAULT_REASON_POLICY = {
  minLength: 12,
  minWords: 3,
  forbidden: ['misc', 'legacy', 'todo', 'n/a', 'none'],
};

const BROAD_EXEMPTION_SKIPS = new Set([
  'all',
  'managed_table',
  'dynamic_model_table',
  'registered_system_table',
]);

const BOUNDED_PROJECTION_EXEMPTION_KIND = 'bounded_projection';

const BOUNDED_PROJECTION_EXEMPT_ACTION = 'bounded_projection_exempt';

function loadConfig(repoRoot, configPath = DEFAULT_CONFIG_FILE) {
  const absolutePath = path.isAbsolute(configPath) ? configPath : path.join(repoRoot, configPath);
  if (!fs.existsSync(absolutePath)) {
    return {};
  }

  return JSON.parse(fs.readFileSync(absolutePath, 'utf8'));
}

function normalizeConfig(config = {}) {
  return {
    tableProfiles: config.tableProfiles || {},
    ownedTableContracts: config.ownedTableContracts || {},
    dynamicModelTablePatterns: (config.dynamicModelTablePatterns || []).map((pattern) => new RegExp(pattern, 'u')),
    registeredSystemTables: new Set(config.registeredSystemTables || []),
    registeredSystemTableTemplates: config.registeredSystemTableTemplates || {},
    appendOnlyTables: new Set(config.appendOnlyTables || []),
    systemScopeTables: new Set(config.systemScopeTables || []),
    defaultTableReadiness: config.defaultTableReadiness || {},
    tableReadiness: config.tableReadiness || {},
    needsOwnerReviewTables: config.needsOwnerReviewTables || {},
    reasonPolicy: {
      ...DEFAULT_REASON_POLICY,
      ...(config.reasonPolicy || {}),
      forbidden: (config.reasonPolicy && config.reasonPolicy.forbidden) || DEFAULT_REASON_POLICY.forbidden,
    },
    exemptions: config.exemptions || {},
  };
}

function finding({ rule, table, message, column = null, severity = 'error' }) {
  return {
    severity,
    rule,
    table: table.name,
    column,
    reason: message,
    message,
    action: actionForRule(rule),
  };
}

function isSkipped(exemption, rule, column = null) {
  if (!exemption || !Array.isArray(exemption.skip)) {
    return false;
  }
  return exemption.skip.includes(rule)
    || (column !== null && exemption.skip.includes(column));
}

function validateExemptionReason(exemption, config) {
  const reason = typeof exemption.reason === 'string' ? exemption.reason.trim() : '';
  if (reason.length === 0) {
    return 'missing';
  }

  const policy = config.reasonPolicy;
  const words = reason.split(/\s+/u).filter(Boolean);
  const normalized = reason.toLowerCase();
  const forbidden = policy.forbidden.some((candidate) => {
    const escaped = candidate.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
    return new RegExp(`(^|[^a-z0-9])${escaped}([^a-z0-9]|$)`, 'u').test(normalized);
  });
  if (reason.length < policy.minLength || words.length < policy.minWords || forbidden) {
    return 'format';
  }

  return null;
}

function collectExemptionPolicyFindings(table, exemption, config) {
  if (!exemption) {
    return [];
  }

  const findings = [];
  const reasonIssue = validateExemptionReason(exemption, config);
  if (reasonIssue === 'missing') {
    findings.push(finding({
      rule: 'exemption-reason-required',
      table,
      message: 'schema hygiene exemptions must include a reason',
    }));
  } else if (reasonIssue === 'format') {
    findings.push(finding({
      rule: 'exemption-reason-format',
      table,
      message: `schema hygiene exemption reason must be specific: min ${config.reasonPolicy.minLength} chars and ${config.reasonPolicy.minWords} words`,
    }));
  }

  const skips = Array.isArray(exemption.skip) ? exemption.skip : [];
  for (const skip of skips) {
    if (BROAD_EXEMPTION_SKIPS.has(skip)) {
      findings.push(finding({
        rule: 'exemption-skip-too-broad',
        table,
        message: `schema hygiene exemption skip must name a concrete rule or column, not ${skip}`,
      }));
    }
  }

  return findings;
}

function exemptionForReport(exemption) {
  if (!exemption) {
    return null;
  }
  const report = {
    reason: exemption.reason || null,
    skip: exemption.skip || [],
  };
  if (typeof exemption.kind === 'string' && exemption.kind.trim().length > 0) {
    report.kind = exemption.kind.trim();
  }
  return report;
}

function applyPlatformReadinessExemption({ platformReadiness, exemption, tableFindings }) {
  if (!exemption || typeof exemption.kind !== 'string' || exemption.kind.trim().length === 0) {
    return platformReadiness;
  }
  if (tableFindings.some((item) => item.severity === 'error')) {
    return platformReadiness;
  }

  const kind = exemption.kind.trim();
  return {
    ...platformReadiness,
    category: kind,
    recommendedActions: [
      kind === BOUNDED_PROJECTION_EXEMPTION_KIND
        ? BOUNDED_PROJECTION_EXEMPT_ACTION
        : `${kind}_declared`,
    ],
    severity: 'ok',
    reason: exemption.reason.trim(),
  };
}

function evaluateManagedTable(table, config, exemption) {
  const findings = [];
  const appendOnly = config.appendOnlyTables.has(table.name);
  const systemScope = config.systemScopeTables.has(table.name);

  if (!hasColumn(table, 'id') && !isSkipped(exemption, 'managed-table-id', 'id')) {
    findings.push(finding({
      rule: 'managed-table-id',
      table,
      column: 'id',
      message: 'managed_table requires an id column',
    }));
  }

  if (!hasColumn(table, 'created_at') && !isSkipped(exemption, 'managed-table-created-at', 'created_at')) {
    findings.push(finding({
      rule: 'managed-table-created-at',
      table,
      column: 'created_at',
      message: 'managed_table requires created_at',
    }));
  }

  if (!appendOnly && !hasColumn(table, 'updated_at') && !isSkipped(exemption, 'managed-table-updated-at-or-append-only', 'updated_at')) {
    findings.push(finding({
      rule: 'managed-table-updated-at-or-append-only',
      table,
      column: 'updated_at',
      message: 'managed_table requires updated_at unless the table is configured append-only',
    }));
  }

  if (!systemScope && !hasScopeColumn(table) && !isSkipped(exemption, 'managed-table-scope-column')) {
    findings.push(finding({
      rule: 'managed-table-scope-column',
      table,
      column: EXPANSION_SCOPE_COLUMN,
      message: 'managed_table requires scope_id for expansion readiness unless configured as system scope',
    }));
  }

  if (!systemScope && hasScopeColumn(table) && !hasScopeTimeIndex(table) && !isSkipped(exemption, 'managed-table-scope-time-index')) {
    findings.push(finding({
      rule: 'managed-table-scope-time-index',
      table,
      message: 'managed_table requires a (scope_id, created_at, id) index for expansion readiness',
    }));
  }

  if (!systemScope && !hasScopeColumn(table) && !isSkipped(exemption, 'managed-table-scope-time-index')) {
    findings.push(finding({
      rule: 'managed-table-scope-time-index',
      table,
      message: 'managed_table cannot satisfy (scope_id, created_at, id) index without scope_id',
    }));
  }

  return findings;
}

function evaluateDynamicModelTable(table, exemption) {
  const findings = [];
  for (const columnName of ['id', 'created_at', 'updated_at']) {
    if (!hasColumn(table, columnName) && !isSkipped(exemption, `dynamic-model-${columnName}`, columnName)) {
      findings.push(finding({
        rule: `dynamic-model-${columnName}`,
        table,
        column: columnName,
        message: `dynamic_model_table requires ${columnName}`,
      }));
    }
  }

  if (!hasScopeColumn(table) && !isSkipped(exemption, 'dynamic-model-scope-column')) {
    findings.push(finding({
      rule: 'dynamic-model-scope-column',
      table,
      column: EXPANSION_SCOPE_COLUMN,
      message: 'dynamic_model_table requires scope_id',
    }));
  }

  if (!hasScopeTimeIndex(table) && !isSkipped(exemption, 'dynamic-model-scope-time-index')) {
    findings.push(finding({
      rule: 'dynamic-model-scope-time-index',
      table,
      message: 'dynamic_model_table requires a (scope_id, created_at, id) index',
    }));
  }

  return findings;
}

function evaluateRegisteredSystemTableTemplate(table, config, exemption) {
  const template = config.registeredSystemTableTemplates[table.name];
  if (!template) {
    return [
      finding({
        rule: 'registered-system-table-template-missing',
        table,
        message: 'registered_system_table requires a fixed field template declaration',
      }),
    ];
  }

  const findings = [];
  for (const columnName of template.requiredColumns || []) {
    if (!hasColumn(table, columnName) && !isSkipped(exemption, 'registered-system-table-required-column', columnName)) {
      findings.push(finding({
        rule: 'registered-system-table-required-column',
        table,
        column: columnName,
        message: `registered_system_table fixed template requires column ${columnName}`,
      }));
    }
  }
  return findings;
}

function evaluateSchemaHygiene({ inventory, config = {} }) {
  const normalizedConfig = normalizeConfig(config);
  const migrationErrors = inventory.migrationErrors || [];
  const findings = [...migrationErrors, ...inventory.parseErrors];
  const tables = inventory.tables.map((table) => {
    const profile = profileForTable(table, normalizedConfig);
    const exemption = normalizedConfig.exemptions[table.name];
    const tableFindings = [];
    const needsOwnerReviewReason = normalizedConfig.needsOwnerReviewTables[table.name];

    tableFindings.push(...collectExemptionPolicyFindings(table, exemption, normalizedConfig));
    tableFindings.push(...ownedTableFindings(table, normalizedConfig.ownedTableContracts[table.name])
      .map((item) => finding({ ...item, table })));

    if (needsOwnerReviewReason) {
      tableFindings.push(finding({
        rule: 'managed-table-needs-owner-review',
        table,
        severity: 'warning',
        message: needsOwnerReviewReason,
      }));
    } else if (profile === 'dynamic_model_table') {
      tableFindings.push(...evaluateDynamicModelTable(table, exemption));
    } else if (profile === 'flow_run_owned_table' || profile === 'retired_archive_table') {
      tableFindings.push(...profileFindingsForTable(table, profile).map((item) => finding({
        ...item,
        table,
      })));
    } else {
      if (profile === 'registered_system_table') {
        tableFindings.push(...evaluateRegisteredSystemTableTemplate(table, normalizedConfig, exemption));
      }
      tableFindings.push(...evaluateManagedTable(table, normalizedConfig, exemption));
    }

    findings.push(...tableFindings);

    const platformReadiness = applyPlatformReadinessExemption({
      platformReadiness: platformReadinessForTable({
        table,
        profile,
        config: normalizedConfig,
        tableFindings,
      }),
      exemption,
      tableFindings,
    });

    return {
      ...table,
      profile,
      exemption: exemptionForReport(exemption),
      platformReadiness,
      findings: tableFindings,
    };
  });

  const errors = findings.filter((candidate) => candidate.severity === 'error').length;
  const warnings = findings.filter((candidate) => candidate.severity === 'warning').length;

  return {
    version: 'schema-hygiene/v1',
    generatedAt: new Date().toISOString(),
    source: inventory.source,
    summary: {
      tables: tables.length,
      findings: findings.length,
      errors,
      warnings,
      migrationErrors: migrationErrors.length,
      parseErrors: inventory.parseErrors.length,
      platformReadiness: {
        ok: tables.filter((table) => table.platformReadiness.severity === 'ok').length,
        warnings: tables.filter((table) => table.platformReadiness.severity === 'warning').length,
        errors: tables.filter((table) => table.platformReadiness.severity === 'error').length,
        needsOwnerReview: tables.filter((table) => (
          table.platformReadiness.recommendedActions.includes('needs_owner_review')
        )).length,
        missingScopeId: tables.filter((table) => !table.platformReadiness.fields.scope_id.present).length,
        missingScopeTimeIdIndex: tables.filter((table) => !table.platformReadiness.hasScopeTimeIdIndex).length,
      },
    },
    tables,
    findings,
  };
}

module.exports = { loadConfig, evaluateSchemaHygiene, DEFAULT_CONFIG_FILE };
