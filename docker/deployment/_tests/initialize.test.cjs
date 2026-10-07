const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { parseEnv } = require('node:util');
const { spawnSync } = require('node:child_process');

function initialize(options) {
  const result = spawnSync('/bin/sh', [path.resolve(__dirname, '../initialize.sh'),
    options.configRoot, options.postgresRoot, `${options.owner.uid}:${options.owner.gid}`,
    ...options.writableDirectories], { env: { PATH: process.env.PATH, ...options.env }, encoding: 'utf8' });
  if (result.status !== 0) throw new Error(result.stderr || result.error?.message || 'Initialization failed');
  return { created: result.stdout.includes('Generated persistent configuration'), file: path.join(options.configRoot, '.env') };
}

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'flowbase-deployment-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const options = {
    configRoot: path.join(root, 'config'),
    postgresRoot: path.join(root, 'postgres'),
    writableDirectories: [path.join(root, 'storage'), path.join(root, 'plugins')],
    owner: { uid: process.getuid(), gid: process.getgid() },
    env: {},
  };
  return { root, options };
}

test('empty deployment generates persistent secrets and the agreed administrator defaults', t => {
  const { options } = fixture(t);
  const result = initialize(options);
  const config = parseEnv(fs.readFileSync(result.file, 'utf8'));
  assert.equal(result.created, true);
  assert.equal(config.BOOTSTRAP_ROOT_ACCOUNT, 'root');
  assert.equal(config.BOOTSTRAP_ROOT_PASSWORD, 'change-me-root-password');
  assert.match(config.POSTGRES_PASSWORD, /^[a-f0-9]{48}$/);
  assert.match(config.API_PROVIDER_SECRET_MASTER_KEY, /^[a-f0-9]{64}$/);
  assert.equal(new URL(config.API_DATABASE_URL).password, config.POSTGRES_PASSWORD);
  assert.equal(fs.readFileSync(path.join(options.configRoot, 'postgres-password'), 'utf8'), config.POSTGRES_PASSWORD);
  assert.equal(fs.statSync(result.file).mode & 0o777, 0o600);
  assert.equal(fs.statSync(path.join(options.configRoot, 'postgres-password')).mode & 0o777, 0o600);
  assert.deepEqual(fs.readdirSync(options.configRoot).sort(), ['.env', 'postgres-password']);
  for (const directory of options.writableDirectories) assert.ok(fs.statSync(directory).isDirectory());
});

test('reinitialization preserves all existing credentials even when defaults change', t => {
  const { options } = fixture(t);
  const result = initialize(options);
  const before = fs.readFileSync(result.file);
  fs.rmSync(path.join(options.configRoot, 'postgres-password'));
  assert.equal(initialize({ ...options, env: { BOOTSTRAP_ROOT_PASSWORD: 'new-default' } }).created, false);
  assert.deepEqual(fs.readFileSync(result.file), before);
  assert.equal(fs.readFileSync(path.join(options.configRoot, 'postgres-password'), 'utf8'), parseEnv(before.toString()).POSTGRES_PASSWORD);
});

test('explicit credentials survive shell loading without command substitution or quote loss', t => {
  const { root, options } = fixture(t);
  const password = "fixture'\"$()`password:with spaces";
  const env = { POSTGRES_PASSWORD: password, API_PROVIDER_SECRET_MASTER_KEY: 'custom-master-key',
    BOOTSTRAP_ROOT_ACCOUNT: 'custom-root', BOOTSTRAP_ROOT_PASSWORD: password };
  const result = initialize({ ...options, env });
  const shell = spawnSync('sh', ['-ec', 'set -a; . "$1"; node -e \'console.log(JSON.stringify(process.env))\'', '--', result.file],
    { cwd: root, encoding: 'utf8' });
  assert.equal(shell.status, 0, shell.stderr);
  const loaded = JSON.parse(shell.stdout);
  for (const [key, value] of Object.entries(env)) assert.equal(loaded[key], value);
  assert.equal(decodeURIComponent(new URL(loaded.API_DATABASE_URL).password), password);
  assert.equal(fs.readFileSync(path.join(options.configRoot, 'postgres-password'), 'utf8'), password);
});

test('existing database without its configuration fails before generating replacement secrets', t => {
  for (const relative of ['PG_VERSION', '18/docker/PG_VERSION']) {
    const { options } = fixture(t);
    const version = path.join(options.postgresRoot, relative);
    fs.mkdirSync(path.dirname(version), { recursive: true });
    fs.writeFileSync(version, '18');
    assert.throws(() => initialize(options), /restore the original config/);
    assert.equal(fs.existsSync(options.configRoot), false);
  }
});

test('incomplete existing configuration is rejected without regenerating its secrets', t => {
  const { options } = fixture(t);
  fs.mkdirSync(options.configRoot);
  const file = path.join(options.configRoot, '.env');
  fs.writeFileSync(file, "POSTGRES_PASSWORD='original'\n");
  assert.throws(() => initialize(options), /Missing API_DATABASE_URL/);
  assert.equal(fs.readFileSync(file, 'utf8'), "POSTGRES_PASSWORD='original'\n");
});
