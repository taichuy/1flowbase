const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');

const PASSWORD_UPDATED_MARKER = '__1FLOWBASE_PASSWORD_UPDATED__';
const ACCOUNT_MISSING_MARKER = '__1FLOWBASE_ACCOUNT_MISSING__';
const SCHEMA_MISSING_MARKER = '__1FLOWBASE_SCHEMA_MISSING__';

function readOptionValue(argv, index, option) {
  const value = argv[index + 1];
  if (value === undefined || value.startsWith('--')) {
    throw new Error(`${option} requires a value`);
  }
  return value;
}

function parseCliArgs(argv, env = process.env) {
  const options = {
    account: env.BOOTSTRAP_ROOT_ACCOUNT || '',
    password: env.BOOTSTRAP_ROOT_PASSWORD || '',
    ifMissing: 'error',
    help: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '-h' || arg === '--help') {
      options.help = true;
      continue;
    }
    if (arg === '--account') {
      options.account = readOptionValue(argv, index, arg);
      index += 1;
      continue;
    }
    if (arg === '--password') {
      options.password = readOptionValue(argv, index, arg);
      index += 1;
      continue;
    }
    if (arg === '--if-missing') {
      options.ifMissing = readOptionValue(argv, index, arg);
      index += 1;
      if (!['error', 'skip'].includes(options.ifMissing)) {
        throw new Error('--if-missing must be `error` or `skip`');
      }
      continue;
    }
    throw new Error(`Unknown option: ${arg}`);
  }
  return options;
}

function base64WithoutPadding(value) {
  return value.toString('base64').replace(/=+$/u, '');
}

function argon2id(message, nonce) {
  return new Promise((resolve, reject) => {
    crypto.argon2('argon2id', {
      message,
      nonce,
      parallelism: 1,
      tagLength: 32,
      memory: 19_456,
      passes: 2,
    }, (error, result) => {
      if (error) {
        reject(error);
        return;
      }
      resolve(result);
    });
  });
}

async function createPasswordHash(password, salt = crypto.randomBytes(16)) {
  if (!password) {
    throw new Error('password is required');
  }
  const hash = await argon2id(Buffer.from(password, 'utf8'), salt);
  return `$argon2id$v=19$m=19456,t=2,p=1$${base64WithoutPadding(salt)}$${base64WithoutPadding(hash)}`;
}

function buildPostgresEnv(databaseUrl) {
  let parsed;
  try {
    parsed = new URL(databaseUrl);
  } catch {
    throw new Error('API_DATABASE_URL must be a valid PostgreSQL URL');
  }
  if (!['postgres:', 'postgresql:'].includes(parsed.protocol)) {
    throw new Error('API_DATABASE_URL must use postgres:// or postgresql://');
  }
  const database = decodeURIComponent(parsed.pathname.replace(/^\/+/, ''));
  if (!parsed.hostname || !database) {
    throw new Error('API_DATABASE_URL must include a host and database name');
  }
  const env = {
    PGDATABASE: database,
    PGHOST: parsed.hostname,
    PGPASSWORD: decodeURIComponent(parsed.password),
    PGPORT: parsed.port || '5432',
    PGUSER: decodeURIComponent(parsed.username || 'postgres'),
  };
  const sslMode = parsed.searchParams.get('sslmode');
  if (sslMode) {
    env.PGSSLMODE = sslMode;
  }
  return env;
}

function runPsql({ args, input, databaseUrl, variables = {}, env = process.env }) {
  return spawnSync('psql', args, {
    encoding: 'utf8',
    env: { ...env, ...buildPostgresEnv(databaseUrl), ...variables },
    input,
    stdio: ['pipe', 'pipe', 'pipe'],
  });
}

const RESET_PASSWORD_SQL = String.raw`\set QUIET 1
\getenv account ONEFLOWBASE_RESET_ACCOUNT
\getenv password_hash ONEFLOWBASE_RESET_PASSWORD_HASH
select to_regclass('public.users') is not null as users_table_exists \gset
\if :users_table_exists
with updated as (
  update public.users
  set password_hash = :'password_hash',
      session_version = session_version + 1,
      updated_by = id,
      updated_at = now()
  where account = :'account'
  returning 1
)
select case when count(*) = 1
  then '${PASSWORD_UPDATED_MARKER}'
  else '${ACCOUNT_MISSING_MARKER}'
end
from updated;
\else
\echo ${SCHEMA_MISSING_MARKER}
\endif
`;

async function resetAccountPassword({
  account,
  password,
  databaseUrl,
  ifMissing = 'error',
  runPsqlImpl = runPsql,
  createPasswordHashImpl = createPasswordHash,
}) {
  if (!account) {
    throw new Error('account is required');
  }
  if (!databaseUrl) {
    throw new Error('API_DATABASE_URL is required');
  }
  const passwordHash = await createPasswordHashImpl(password);
  const args = ['-X', '-A', '-t', '-v', 'ON_ERROR_STOP=1'];
  const result = runPsqlImpl({
    args,
    input: RESET_PASSWORD_SQL,
    databaseUrl,
    variables: {
      ONEFLOWBASE_RESET_ACCOUNT: account,
      ONEFLOWBASE_RESET_PASSWORD_HASH: passwordHash,
    },
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    const detail = String(result.stderr || result.stdout || '').trim();
    throw new Error(`psql failed with exit code ${result.status}${detail ? `: ${detail}` : ''}`);
  }
  const output = String(result.stdout || '');
  if (output.includes(PASSWORD_UPDATED_MARKER)) {
    return { status: 'updated', account };
  }
  if (output.includes(ACCOUNT_MISSING_MARKER) || output.includes(SCHEMA_MISSING_MARKER)) {
    if (ifMissing === 'skip') {
      return { status: 'skipped', account };
    }
    throw new Error(`account not found: ${account}`);
  }
  throw new Error('psql completed without a password reset result');
}

module.exports = {
  buildPostgresEnv,
  createPasswordHash,
  parseCliArgs,
  resetAccountPassword,
  runPsql,
};
