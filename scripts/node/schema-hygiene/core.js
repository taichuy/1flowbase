const path = require('node:path');
const { splitSqlStatements, splitTopLevel } = require("./ddl-parser.js");
const { getRepoRoot, normalizePath, collectSchemaInventory, DEFAULT_MIGRATIONS_DIR } = require("./migration-inventory.js");
const { writeReports, DEFAULT_MAX_FINDINGS } = require("./reports.js");
const { loadConfig, evaluateSchemaHygiene, DEFAULT_CONFIG_FILE } = require("./schema-policy.js");

function parseSchemaHygieneCliArgs(argv) {
  const options = {
    help: false,
    maxFindings: DEFAULT_MAX_FINDINGS,
    migrationsDir: DEFAULT_MIGRATIONS_DIR,
    configPath: DEFAULT_CONFIG_FILE,
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
    if (arg === '--config') {
      options.configPath = argv[index + 1];
      index += 1;
      continue;
    }
    throw new Error(`Unknown schema-hygiene option: ${arg}`);
  }

  if (!Number.isInteger(options.maxFindings) || options.maxFindings < 1) {
    throw new Error('--max-findings must be a positive integer');
  }

  return options;
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/tooling.js schema-hygiene [--max-findings <n>] [--migrations-dir <path>] [--config <path>]\n'
      + 'Scans PostgreSQL migrations for schema inventory and expansion-friendly hygiene rules.\n'
  );
}

async function main(argv = [], deps = {}) {
  const options = parseSchemaHygieneCliArgs(argv);
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));
  const writeStderr = deps.writeStderr || ((text) => process.stderr.write(text));

  if (options.help) {
    usage(writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const inventory = (deps.collectSchemaInventoryImpl || collectSchemaInventory)({
    repoRoot,
    migrationsDir: options.migrationsDir,
  });
  const config = deps.config || loadConfig(repoRoot, options.configPath);
  const report = evaluateSchemaHygiene({ inventory, config });
  const { jsonPath, markdownPath } = writeReports({
    repoRoot,
    report,
    maxFindings: options.maxFindings,
  });

  writeStdout(
    `[1flowbase-schema-hygiene] ${report.summary.findings} findings `
      + `(${report.summary.errors} errors, ${report.summary.warnings} warnings). `
      + `Reports: ${normalizePath(path.relative(repoRoot, jsonPath))}, `
      + `${normalizePath(path.relative(repoRoot, markdownPath))}\n`
  );

  for (const item of report.findings.filter((candidate) => candidate.severity === 'error')) {
    writeStderr(
      `[schema-hygiene:${item.rule}] ${item.table || item.file || 'schema'} ${item.message}\n`
    );
  }

  return report.summary.errors > 0 ? 1 : 0;
}

module.exports = {
  collectSchemaInventory,
  evaluateSchemaHygiene,
  loadConfig,
  main,
  parseSchemaHygieneCliArgs,
  splitSqlStatements,
  splitTopLevel,
  writeReports,
};
