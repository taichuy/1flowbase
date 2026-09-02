#!/usr/bin/env node

const path = require('node:path');

const { parseEnvFile, parseApiEnvironment } = require('../dev-up/env.js');
const { parseCliArgs, resetAccountPassword } = require('./core.js');
const { validateAppliedMigrations } = require('./migration-validation.js');

function usage(write = (value) => process.stdout.write(value)) {
  write(`Usage: node scripts/node/reset-account-password.js [options]

Options:
  --account <account>       Account to reset; defaults to BOOTSTRAP_ROOT_ACCOUNT
  --password <password>     New password; defaults to BOOTSTRAP_ROOT_PASSWORD
  --if-missing error|skip   Missing account behavior; defaults to error
  -h, --help                Show this help
`);
}

async function main(argv = process.argv.slice(2), sourceEnv = process.env) {
  const repoRoot = path.resolve(__dirname, '..', '..', '..');
  const envFile = path.join(repoRoot, 'api', 'apps', 'api-server', '.env');
  const env = { ...parseEnvFile(envFile), ...sourceEnv };
  const options = parseCliArgs(argv, env);
  if (options.help) {
    usage();
    return 0;
  }
  if (parseApiEnvironment(env.API_ENV) === 'production') {
    throw new Error('automatic account password reset is disabled in production');
  }
  validateAppliedMigrations({ repoRoot, databaseUrl: env.API_DATABASE_URL });
  const result = await resetAccountPassword({
    ...options,
    databaseUrl: env.API_DATABASE_URL,
  });
  if (result.status === 'updated') {
    process.stdout.write(`reset password for account ${result.account}\n`);
  } else {
    process.stdout.write(`account ${result.account} is not initialized; API bootstrap will own creation\n`);
  }
  return 0;
}

if (require.main === module) {
  main().then(
    (exitCode) => {
      process.exitCode = exitCode;
    },
    (error) => {
      process.stderr.write(`[1flowbase-reset-account-password] ${error.message}\n`);
      process.exitCode = 1;
    }
  );
}

module.exports = { main, usage };
