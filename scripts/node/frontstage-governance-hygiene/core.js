const fs = require('node:fs');
const path = require('node:path');

const OUTPUT_ROOT = path.join('tmp', 'test-governance');
const JSON_REPORT_FILE = 'frontstage-governance-hygiene.json';
const MARKDOWN_REPORT_FILE = 'frontstage-governance-hygiene.md';
const DEFAULT_MAX_FINDINGS = 200;
const DEFAULT_MIGRATIONS_DIR = path.join(
  'api',
  'crates',
  'storage-durable',
  'postgres',
  'migrations'
);
const FRONTSTAGE_SERVICE_FILE = path.join(
  'api',
  'crates',
  'control-plane',
  'src',
  'frontstage',
  'mod.rs'
);
const FRONTSTAGE_REPOSITORY_FILE = path.join(
  'api',
  'crates',
  'storage-durable',
  'postgres',
  'src',
  'frontstage_repository.rs'
);
const BACKEND_SETTINGS_ROUTES_FILE = path.join(
  'api',
  'crates',
  'access-control',
  'src',
  'settings_routes.rs'
);
const FRONTEND_SETTINGS_SECTIONS_FILE = path.join(
  'web',
  'app',
  'src',
  'features',
  'settings',
  'lib',
  'settings-sections.tsx'
);
const VISIBILITY_GATED_READ_SERVICE_METHODS = [
  'get_page_detail',
  'get_block_code',
];

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function normalizePath(filePath) {
  return filePath.split(path.sep).join('/');
}

function readRequiredFile(repoRoot, relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');
}

function collectSqlFiles(rootDir) {
  if (!fs.existsSync(rootDir)) {
    return [];
  }

  const entries = fs.readdirSync(rootDir, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const absolutePath = path.join(rootDir, entry.name);
    if (entry.isDirectory()) {
      return collectSqlFiles(absolutePath);
    }
    if (!entry.isFile() || !entry.name.endsWith('.sql')) {
      return [];
    }
    return [absolutePath];
  });
}

function collectMigrationSources(repoRoot, migrationsDir = DEFAULT_MIGRATIONS_DIR) {
  const absoluteDir = path.isAbsolute(migrationsDir)
    ? migrationsDir
    : path.join(repoRoot, migrationsDir);

  return collectSqlFiles(absoluteDir)
    .sort()
    .map((absolutePath) => ({
      file: normalizePath(path.relative(repoRoot, absolutePath)),
      source: fs.readFileSync(absolutePath, 'utf8'),
    }));
}

function normalizeSql(source) {
  return source
    .replace(/--.*$/gmu, ' ')
    .replace(/\s+/gu, ' ')
    .toLowerCase();
}

function escapeRegExp(input) {
  return input.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
}

function hasForeignKey({ sql, column, targetTable, targetColumn }) {
  const normalized = normalizeSql(sql);
  const columnPattern = escapeRegExp(column);
  const targetTablePattern = escapeRegExp(targetTable);
  const targetColumnPattern = escapeRegExp(targetColumn);

  const inlinePattern = new RegExp(
    `${columnPattern}\\s+uuid\\b[^,;]*\\breferences\\s+${targetTablePattern}\\s*\\(\\s*${targetColumnPattern}\\s*\\)`,
    'u'
  );
  const tableConstraintPattern = new RegExp(
    `foreign\\s+key\\s*\\(\\s*${columnPattern}\\s*\\)\\s+references\\s+${targetTablePattern}\\s*\\(\\s*${targetColumnPattern}\\s*\\)`,
    'u'
  );
  const compositeConstraintPattern = new RegExp(
    `foreign\\s+key\\s*\\([^)]*\\b${columnPattern}\\b[^)]*\\)\\s+references\\s+${targetTablePattern}\\s*\\([^)]*\\b${targetColumnPattern}\\b[^)]*\\)`,
    'u'
  );

  return (
    inlinePattern.test(normalized)
    || tableConstraintPattern.test(normalized)
    || compositeConstraintPattern.test(normalized)
  );
}

function columnDefinition(sql, table, column) {
  const normalized = normalizeSql(sql);
  const tableIndex = normalized.indexOf(`create table if not exists ${table}`);
  const fallbackIndex = tableIndex === -1
    ? normalized.indexOf(`create table ${table}`)
    : tableIndex;

  if (fallbackIndex === -1) {
    return null;
  }

  const rest = normalized.slice(fallbackIndex);
  const columnPattern = new RegExp(
    `\\b${escapeRegExp(column)}\\s+uuid\\b([^,)]*)`,
    'u'
  );
  return columnPattern.exec(rest)?.[0] ?? null;
}

function hasCreateTable(sql, table) {
  return new RegExp(
    `\\bcreate\\s+table\\s+(?:if\\s+not\\s+exists\\s+)?${escapeRegExp(table)}\\b`,
    'iu'
  ).test(sql);
}

function hasWorkspaceParentQuery({ serviceSource, repositorySource }) {
  const ensureParentBody = extractRustMethodBody(serviceSource, 'ensure_page_parent');
  const getPageBody = extractRustMethodBody(repositorySource, 'get_frontstage_page');

  if (!ensureParentBody || !getPageBody) {
    return false;
  }

  const callsWorkspaceBoundLookup = /get_frontstage_page\s*\(\s*workspace_id\s*,\s*parent_id/u
    .test(ensureParentBody);
  const enforcesGroupParent = /FrontstagePageKind::Group/u.test(ensureParentBody);
  const repositoryScopesByWorkspace =
    /where\s+workspace_id\s*=\s*\$\d+\s+and\s+id\s*=\s*\$\d+/iu.test(getPageBody)
    || /where\s+[^;]*\bworkspace_id\b[^;]*\bid\b/iu.test(getPageBody);

  return callsWorkspaceBoundLookup && enforcesGroupParent && repositoryScopesByWorkspace;
}

function hasMigrationCycleProof(sql) {
  const normalized = normalizeSql(sql);
  return (
    /prevent[_\s]+frontstage[_\s]+page[_\s]+cycle/u.test(normalized)
    || (
      /with\s+recursive/u.test(normalized)
      && /frontstage_pages/u.test(normalized)
      && /\bparent_id\b/u.test(normalized)
    )
  );
}

function hasRootAwareVisibilityUnique(sql) {
  const normalized = normalizeSql(sql);
  const partialRootUnique = new RegExp(
    `unique\\s+index\\b[^;]*\\bon\\s+frontstage_page_visibility_rules\\s*\\(\\s*workspace_id\\s*,\\s*role_id\\s*\\)[^;]*where\\s+page_id\\s+is\\s+null`,
    'u'
  ).test(normalized);
  const partialPageUnique = new RegExp(
    `unique\\s+index\\b[^;]*\\bon\\s+frontstage_page_visibility_rules\\s*\\(\\s*workspace_id\\s*,\\s*page_id\\s*,\\s*role_id\\s*\\)[^;]*where\\s+page_id\\s+is\\s+not\\s+null`,
    'u'
  ).test(normalized);
  const coalesceUnique =
    /unique\s+index\b[^;]*\bon\s+frontstage_page_visibility_rules\b[^;]*coalesce\s*\(\s*page_id/u
      .test(normalized)
    && /\bworkspace_id\b/u.test(normalized)
    && /\brole_id\b/u.test(normalized);
  const nullsNotDistinct =
    /frontstage_page_visibility_rules\b[^;]*(unique|nulls\s+not\s+distinct)[^;]*nulls\s+not\s+distinct[^;]*\(\s*workspace_id\s*,\s*page_id\s*,\s*role_id\s*\)/u
      .test(normalized)
    || /unique\s+nulls\s+not\s+distinct\s*\(\s*workspace_id\s*,\s*page_id\s*,\s*role_id\s*\)/u
      .test(normalized);

  return (partialRootUnique && partialPageUnique) || coalesceUnique || nullsNotDistinct;
}

function findMatchingBrace(source, openingBraceIndex) {
  let depth = 0;
  let quote = null;
  let escaped = false;

  for (let index = openingBraceIndex; index < source.length; index += 1) {
    const char = source[index];
    const next = source[index + 1];

    if (quote) {
      if (escaped) {
        escaped = false;
        continue;
      }
      if (char === '\\') {
        escaped = true;
        continue;
      }
      if (char === quote) {
        quote = null;
      }
      continue;
    }

    if (char === '"' || char === "'") {
      quote = char;
      continue;
    }

    if (char === '/' && next === '/') {
      const lineEnd = source.indexOf('\n', index + 2);
      index = lineEnd === -1 ? source.length : lineEnd;
      continue;
    }

    if (char === '{') {
      depth += 1;
      continue;
    }

    if (char === '}') {
      depth -= 1;
      if (depth === 0) {
        return index;
      }
    }
  }

  return -1;
}

function extractRustMethodBody(source, methodName) {
  const pattern = new RegExp(
    `\\b(?:pub\\s+)?(?:async\\s+)?fn\\s+${escapeRegExp(methodName)}\\s*\\(`,
    'u'
  );
  const match = pattern.exec(source);
  if (!match) {
    return null;
  }

  const openingBraceIndex = source.indexOf('{', match.index);
  if (openingBraceIndex === -1) {
    return null;
  }

  const closingBraceIndex = findMatchingBrace(source, openingBraceIndex);
  if (closingBraceIndex === -1) {
    return null;
  }

  return source.slice(openingBraceIndex + 1, closingBraceIndex);
}

function extractSettingsRouteSpecsBody(source) {
  const match = source.match(
    /const\s+SETTINGS_ROUTE_SPECS:\s*&\[\s*SettingsRouteSpec\s*\]\s*=\s*&\[(.*?)\];/su
  );
  return match?.[1] ?? null;
}

function parseBackendSettingsRoutePaths(source) {
  const specsBody = extractSettingsRouteSpecsBody(source);
  if (!specsBody) {
    return null;
  }

  return Array.from(specsBody.matchAll(/\bpath:\s*"([^"]+)"/gu), (match) => match[1]);
}

function extractFrontendSettingsSectionsBody(source) {
  const match = source.match(
    /export\s+const\s+settingsSectionDefinitions:\s*SettingsSectionDefinition\[\]\s*=\s*\[(.*?)\];/su
  );
  return match?.[1] ?? null;
}

function parseFrontendSettingsSectionPaths(source) {
  const sectionsBody = extractFrontendSettingsSectionsBody(source);
  if (!sectionsBody) {
    return null;
  }

  return Array.from(sectionsBody.matchAll(/\bto:\s*['"]([^'"]+)['"]/gu), (match) => match[1]);
}

function collectFrontstageGovernanceInventory({
  repoRoot = getRepoRoot(),
  migrationsDir = DEFAULT_MIGRATIONS_DIR,
  frontstageServicePath = FRONTSTAGE_SERVICE_FILE,
  frontstageRepositoryPath = FRONTSTAGE_REPOSITORY_FILE,
  backendSettingsRoutesPath = BACKEND_SETTINGS_ROUTES_FILE,
  frontendSettingsSectionsPath = FRONTEND_SETTINGS_SECTIONS_FILE,
} = {}) {
  const migrationSources = collectMigrationSources(repoRoot, migrationsDir);
  const migrationSql = migrationSources.map((migration) => migration.source).join('\n');
  const serviceSource = readRequiredFile(repoRoot, frontstageServicePath);
  const repositorySource = readRequiredFile(repoRoot, frontstageRepositoryPath);
  const backendSettingsRoutesSource = readRequiredFile(repoRoot, backendSettingsRoutesPath);
  const frontendSettingsSectionsSource = readRequiredFile(repoRoot, frontendSettingsSectionsPath);

  return {
    paths: {
      migrationsDir,
      frontstageServicePath,
      frontstageRepositoryPath,
      backendSettingsRoutesPath,
      frontendSettingsSectionsPath,
    },
    migrationSources,
    migrationSql,
    serviceSource,
    repositorySource,
    backendSettingsRoutePaths: parseBackendSettingsRoutePaths(backendSettingsRoutesSource),
    frontendSettingsSectionPaths: parseFrontendSettingsSectionPaths(frontendSettingsSectionsSource),
  };
}

function createFinding({
  rule,
  message,
  sectionKey,
  file,
  severity = 'error',
}) {
  return {
    rule,
    severity,
    sectionKey,
    file,
    message,
  };
}

function evaluateTreeGovernance({ inventory, findings }) {
  const migrationSql = inventory.migrationSql;
  const hasParentFk = hasForeignKey({
    sql: migrationSql,
    column: 'parent_id',
    targetTable: 'frontstage_pages',
    targetColumn: 'id',
  });

  if (!hasParentFk) {
    findings.push(createFinding({
      rule: 'frontstage-pages-parent-fk',
      sectionKey: 'frontstage_pages',
      file: inventory.paths.migrationsDir,
      message:
        'frontstage_pages.parent_id must reference frontstage_pages(id) to reject orphan parents at the database boundary',
    }));
  }

  if (!hasWorkspaceParentQuery({
    serviceSource: inventory.serviceSource,
    repositorySource: inventory.repositorySource,
  })) {
    findings.push(createFinding({
      rule: 'frontstage-pages-workspace-parent-boundary',
      sectionKey: 'frontstage_pages',
      file: inventory.paths.frontstageServicePath,
      message:
        'frontstage page parent validation must look up parent_id through workspace_id and only allow group parents',
    }));
  }

  if (!hasMigrationCycleProof(migrationSql)) {
    findings.push(createFinding({
      rule: 'frontstage-page-tree-cycle-static-proof',
      sectionKey: 'frontstage_pages',
      file: inventory.paths.migrationsDir,
      severity: 'warning',
      message:
        'static scan did not find a migration-level recursive or trigger proof for cycle rejection; keep service-level parent guards and route tests as the executable proof',
    }));
  }
}

function evaluateVisibilityRuleMigrations({ inventory, findings }) {
  const migrationSql = inventory.migrationSql;
  const table = 'frontstage_page_visibility_rules';
  const hasTable = hasCreateTable(migrationSql, table);
  const pageColumn = columnDefinition(migrationSql, table, 'page_id');
  const roleColumn = columnDefinition(migrationSql, table, 'role_id');

  if (!hasTable) {
    findings.push(createFinding({
      rule: 'frontstage-page-visibility-table',
      sectionKey: table,
      file: inventory.paths.migrationsDir,
      message: 'frontstage_page_visibility_rules must be declared by storage-durable migrations',
    }));
  }

  if (
    !pageColumn
    || /\bnot\s+null\b/u.test(pageColumn)
    || !hasForeignKey({
      sql: migrationSql,
      column: 'page_id',
      targetTable: 'frontstage_pages',
      targetColumn: 'id',
    })
  ) {
    findings.push(createFinding({
      rule: 'frontstage-page-visibility-page-fk',
      sectionKey: table,
      file: inventory.paths.migrationsDir,
      message:
        'frontstage_page_visibility_rules.page_id must be nullable for root rules and must reference frontstage_pages(id)',
    }));
  }

  if (
    !roleColumn
    || !/\bnot\s+null\b/u.test(roleColumn)
    || !hasForeignKey({
      sql: migrationSql,
      column: 'role_id',
      targetTable: 'roles',
      targetColumn: 'id',
    })
  ) {
    findings.push(createFinding({
      rule: 'frontstage-page-visibility-role-fk',
      sectionKey: table,
      file: inventory.paths.migrationsDir,
      message:
        'frontstage_page_visibility_rules.role_id must be not null and must reference roles(id)',
    }));
  }

  if (!hasRootAwareVisibilityUnique(migrationSql)) {
    findings.push(createFinding({
      rule: 'frontstage-page-visibility-root-rule-unique',
      sectionKey: table,
      file: inventory.paths.migrationsDir,
      message:
        'visibility rules must enforce one row per workspace_id + page_id + role_id, including NULL page_id root rules via partial indexes, coalesce, or NULLS NOT DISTINCT',
    }));
  }
}

function evaluateServiceVisibilityGates({ inventory, findings }) {
  for (const methodName of VISIBILITY_GATED_READ_SERVICE_METHODS) {
    const body = extractRustMethodBody(inventory.serviceSource, methodName);
    if (!body || !/\bensure_page_visible\s*\(/u.test(body)) {
      findings.push(createFinding({
        rule: 'frontstage-page-service-visibility-gate',
        sectionKey: methodName,
        file: inventory.paths.frontstageServicePath,
        message:
          `FrontstagePageService.${methodName} must call ensure_page_visible before reading page detail/content/block-code data`,
      }));
    }
  }
}

function evaluateSettingsRegistry({ inventory, findings }) {
  if (!inventory.backendSettingsRoutePaths) {
    findings.push(createFinding({
      rule: 'backend-settings-registry-unreadable',
      sectionKey: 'SETTINGS_ROUTE_SPECS',
      file: inventory.paths.backendSettingsRoutesPath,
      message: 'unable to locate backend SETTINGS_ROUTE_SPECS for dynamic frontstage page check',
    }));
  } else {
    for (const routePath of inventory.backendSettingsRoutePaths) {
      if (routePath.startsWith('/frontstage/pages/')) {
        findings.push(createFinding({
          rule: 'backend-settings-registry-dynamic-frontstage-page',
          sectionKey: 'SETTINGS_ROUTE_SPECS',
          file: inventory.paths.backendSettingsRoutesPath,
          message:
            `dynamic frontstage page route "${routePath}" must not be registered in backend SETTINGS_ROUTE_SPECS`,
        }));
      }
    }
  }

  if (!inventory.frontendSettingsSectionPaths) {
    findings.push(createFinding({
      rule: 'frontend-settings-registry-unreadable',
      sectionKey: 'settingsSectionDefinitions',
      file: inventory.paths.frontendSettingsSectionsPath,
      message:
        'unable to locate frontend settingsSectionDefinitions for dynamic frontstage page check',
    }));
  } else {
    for (const routePath of inventory.frontendSettingsSectionPaths) {
      if (routePath.startsWith('/frontstage/pages/')) {
        findings.push(createFinding({
          rule: 'frontend-settings-registry-dynamic-frontstage-page',
          sectionKey: 'settingsSectionDefinitions',
          file: inventory.paths.frontendSettingsSectionsPath,
          message:
            `dynamic frontstage page route "${routePath}" must not be registered in frontend settingsSectionDefinitions`,
        }));
      }
    }
  }
}

function evaluateFrontstageGovernanceHygiene({ inventory }) {
  const findings = [];

  evaluateTreeGovernance({ inventory, findings });
  evaluateVisibilityRuleMigrations({ inventory, findings });
  evaluateServiceVisibilityGates({ inventory, findings });
  evaluateSettingsRegistry({ inventory, findings });

  return {
    generatedAt: new Date().toISOString(),
    summary: {
      findings: findings.length,
      errors: findings.filter((finding) => finding.severity === 'error').length,
      warnings: findings.filter((finding) => finding.severity === 'warning').length,
    },
    findings,
  };
}

function renderMarkdown(report) {
  const lines = [
    '# Frontstage Governance Hygiene',
    '',
    `- Findings: ${report.summary.findings}`,
    `- Errors: ${report.summary.errors}`,
    `- Warnings: ${report.summary.warnings}`,
    '',
  ];

  if (report.findings.length === 0) {
    lines.push('No findings.');
    return `${lines.join('\n')}\n`;
  }

  lines.push('| Severity | Rule | Section | File | Message |');
  lines.push('| --- | --- | --- | --- | --- |');

  for (const finding of report.findings) {
    const message = finding.message
      .replace(/\|/gu, '\\|')
      .replace(/\n/gu, '<br>');
    lines.push(
      `| ${finding.severity} | ${finding.rule} | ${finding.sectionKey} | ${finding.file} | ${message} |`
    );
  }

  return `${lines.join('\n')}\n`;
}

function writeReport({ repoRoot, report, maxFindings = DEFAULT_MAX_FINDINGS }) {
  const outputDir = path.join(repoRoot, OUTPUT_ROOT);
  fs.mkdirSync(outputDir, { recursive: true });

  const reportForDisk = {
    ...report,
    findings: report.findings.slice(0, maxFindings),
    truncated: report.findings.length > maxFindings,
  };

  const jsonReportPath = path.join(outputDir, JSON_REPORT_FILE);
  fs.writeFileSync(
    jsonReportPath,
    `${JSON.stringify(reportForDisk, null, 2)}\n`,
    'utf8'
  );

  const markdownReportPath = path.join(outputDir, MARKDOWN_REPORT_FILE);
  fs.writeFileSync(markdownReportPath, renderMarkdown(reportForDisk), 'utf8');

  return {
    jsonReportPath,
    markdownReportPath,
    report: reportForDisk,
  };
}

function parseCliArgs(argv) {
  const options = {
    help: false,
    maxFindings: DEFAULT_MAX_FINDINGS,
    migrationsDir: DEFAULT_MIGRATIONS_DIR,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '-h' || arg === '--help') {
      options.help = true;
      continue;
    }

    if (arg === '--max-findings') {
      options.maxFindings = Number.parseInt(argv[index + 1], 10);
      index += 1;
      continue;
    }

    if (arg === '--migrations-dir') {
      options.migrationsDir = argv[index + 1];
      index += 1;
      continue;
    }

    throw new Error(`Unknown frontstage-governance-hygiene option: ${arg}`);
  }

  if (!Number.isInteger(options.maxFindings) || options.maxFindings < 1) {
    throw new Error('--max-findings must be a positive integer');
  }

  if (!options.migrationsDir) {
    throw new Error('--migrations-dir must not be empty');
  }

  return options;
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/tooling.js frontstage-governance-hygiene '
      + '[--max-findings <n>] [--migrations-dir <path>]\n'
      + 'Checks frontstage page tree constraints, visibility rule migrations, '
      + 'service visibility gates, and settings route registry boundaries.\n'
  );
}

async function main(argv = [], deps = {}) {
  const options = parseCliArgs(argv);
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));
  const writeStderr = deps.writeStderr || ((text) => process.stderr.write(text));

  if (options.help) {
    usage(writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const inventory = (deps.collectInventoryImpl || collectFrontstageGovernanceInventory)({
    repoRoot,
    migrationsDir: options.migrationsDir,
  });
  const report = (deps.evaluateImpl || evaluateFrontstageGovernanceHygiene)({
    inventory,
  });
  const {
    jsonReportPath,
    markdownReportPath,
    report: reportForDisk,
  } = writeReport({
    repoRoot,
    report,
    maxFindings: options.maxFindings,
  });

  writeStdout(
    `[1flowbase-frontstage-governance-hygiene] ${report.summary.findings} findings `
      + `(${report.summary.errors} errors, ${report.summary.warnings} warnings). `
      + `Reports: ${normalizePath(path.relative(repoRoot, jsonReportPath))}, `
      + `${normalizePath(path.relative(repoRoot, markdownReportPath))}\n`
  );

  for (const finding of reportForDisk.findings.filter((item) => item.severity === 'error')) {
    writeStderr(
      `[frontstage-governance-hygiene:${finding.rule}] ${finding.sectionKey} `
        + `${finding.message}\n`
    );
  }

  return report.summary.errors > 0 ? 1 : 0;
}

module.exports = {
  collectFrontstageGovernanceInventory,
  evaluateFrontstageGovernanceHygiene,
  extractRustMethodBody,
  main,
  parseCliArgs,
  writeReport,
};
