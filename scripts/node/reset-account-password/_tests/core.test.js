const test = require('node:test');
const assert = require('node:assert/strict');

const {
  buildPostgresEnv,
  createPasswordHash,
  parseCliArgs,
  resetAccountPassword,
} = require('../core.js');

test('AC-002 creates a Rust-compatible Argon2id PHC password hash', async () => {
  const passwordHash = await createPasswordHash('x', Buffer.alloc(16, 1));

  assert.equal(
    passwordHash,
    '$argon2id$v=19$m=19456,t=2,p=1$AQEBAQEBAQEBAQEBAQEBAQ$+m21d7tbteefuH7Lm+E+MLLzRJL8zllqwBoWL8clQi0'
  );
});

test('AC-002 maps the backend database URL to libpq environment without exposing credentials in args', () => {
  assert.deepEqual(
    buildPostgresEnv('postgres://app:p%40ss@127.0.0.1:55432/flowbase?sslmode=disable'),
    {
      PGDATABASE: 'flowbase',
      PGHOST: '127.0.0.1',
      PGPASSWORD: 'p@ss',
      PGPORT: '55432',
      PGSSLMODE: 'disable',
      PGUSER: 'app',
    }
  );
});

test('AC-003 CLI defaults to root credentials and requires explicit missing policy', () => {
  assert.deepEqual(
    parseCliArgs([], {
      BOOTSTRAP_ROOT_ACCOUNT: 'root',
      BOOTSTRAP_ROOT_PASSWORD: 'change-me',
    }),
    { account: 'root', password: 'change-me', ifMissing: 'error', help: false }
  );
  assert.equal(
    parseCliArgs(['--account', 'alice', '--password', 'secret', '--if-missing', 'skip'], {}).ifMissing,
    'skip'
  );
});

test('AC-002 resets one account and invalidates existing sessions in one SQL statement', async () => {
  const calls = [];
  const result = await resetAccountPassword({
    account: 'alice',
    password: 'secret',
    databaseUrl: 'postgres://app:db-secret@localhost/flowbase',
    runPsqlImpl(input) {
      calls.push(input);
      return { status: 0, stdout: '__1FLOWBASE_PASSWORD_UPDATED__\n', stderr: '' };
    },
    createPasswordHashImpl: async () => '$argon2id$test',
  });

  assert.deepEqual(result, { status: 'updated', account: 'alice' });
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].args, ['-X', '-A', '-t', '-v', 'ON_ERROR_STOP=1']);
  assert.deepEqual(calls[0].variables, {
    ONEFLOWBASE_RESET_ACCOUNT: 'alice',
    ONEFLOWBASE_RESET_PASSWORD_HASH: '$argon2id$test',
  });
  assert.doesNotMatch(calls[0].args.join(' '), /db-secret/u);
  assert.match(calls[0].input, /session_version = session_version \+ 1/u);
  assert.match(calls[0].input, /updated_by = id/u);
});

test('AC-003 missing accounts fail by default and may be explicitly skipped by dev-up', async () => {
  const missing = {
    account: 'missing',
    password: 'secret',
    databaseUrl: 'postgres://postgres:secret@localhost/flowbase',
    runPsqlImpl: () => ({ status: 0, stdout: '__1FLOWBASE_ACCOUNT_MISSING__\n', stderr: '' }),
    createPasswordHashImpl: async () => '$argon2id$test',
  };

  await assert.rejects(() => resetAccountPassword(missing), /account not found: missing/u);
  assert.deepEqual(
    await resetAccountPassword({ ...missing, ifMissing: 'skip' }),
    { status: 'skipped', account: 'missing' }
  );
});

test('AC-004 surfaces database failures without echoing the configured password', async () => {
  await assert.rejects(
    () => resetAccountPassword({
      account: 'alice',
      password: 'new-secret',
      databaseUrl: 'postgres://postgres:db-secret@localhost/flowbase',
      runPsqlImpl: () => ({ status: 2, stdout: '', stderr: 'connection refused' }),
      createPasswordHashImpl: async () => '$argon2id$test',
    }),
    (error) => {
      assert.match(error.message, /psql failed with exit code 2: connection refused/u);
      assert.doesNotMatch(error.message, /new-secret|db-secret/u);
      return true;
    }
  );
});
