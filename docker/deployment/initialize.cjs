#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');

function shellQuote(value) {
  return `'${value.replaceAll("'", "'\\''")}'`;
}

function existingDatabase(root) {
  if (fs.existsSync(path.join(root, 'PG_VERSION'))) return true;
  if (!fs.existsSync(root)) return false;
  return fs.readdirSync(root).some(version => /^\d+$/.test(version) &&
    fs.existsSync(path.join(root, version, 'docker', 'PG_VERSION')));
}

function createConfig(file, text, owner) {
  const temporary = `${file}.${crypto.randomUUID()}.tmp`;
  try {
    fs.writeFileSync(temporary, text, { mode: 0o600, flag: 'wx' });
    fs.chownSync(temporary, owner.uid, owner.gid);
    // Publish a complete file without replacing another initializer's result.
    try { fs.linkSync(temporary, file); }
    catch (error) { if (error.code !== 'EEXIST') throw error; }
  } finally {
    fs.rmSync(temporary, { force: true });
  }
}

function initialize({
  configRoot = '/config',
  postgresRoot = '/pgdata',
  writableDirectories = ['/app/api/storage', '/app/api/plugins/packages',
    '/app/api/plugins/installed', '/app/api/plugins/host-extension/dropins'],
  owner = fs.statSync('/home/flowbase'),
  env = process.env,
} = {}) {
  const file = path.join(configRoot, '.env');
  if (!fs.existsSync(file) && existingDatabase(postgresRoot)) {
    throw new Error('Existing database without deployment configuration; restore the original config');
  }
  for (const directory of [configRoot, ...writableDirectories]) {
    fs.mkdirSync(directory, { recursive: true });
    fs.chownSync(directory, owner.uid, owner.gid);
  }
  const created = !fs.existsSync(file);
  if (created) {
    const password = env.POSTGRES_PASSWORD || crypto.randomBytes(24).toString('hex');
    const values = {
      POSTGRES_PASSWORD: password,
      API_DATABASE_URL: `postgres://postgres:${encodeURIComponent(password)}@db:5432/1flowbase`,
      API_PROVIDER_SECRET_MASTER_KEY: env.API_PROVIDER_SECRET_MASTER_KEY || crypto.randomBytes(32).toString('hex'),
      BOOTSTRAP_ROOT_ACCOUNT: env.BOOTSTRAP_ROOT_ACCOUNT || 'root',
      BOOTSTRAP_ROOT_PASSWORD: env.BOOTSTRAP_ROOT_PASSWORD || 'change-me-root-password',
    };
    createConfig(file, Object.entries(values).map(([key, value]) => `${key}=${shellQuote(value)}\n`).join(''), owner);
  }
  // Read with the same POSIX shell semantics used by api-start.sh, including
  // embedded quotes. Start with an empty environment so missing keys stay missing.
  const loaded = spawnSync('/bin/sh', ['-ec',
    'set -a; . "$1"; "$2" -e \'console.log(JSON.stringify(process.env))\'',
    '--', file, process.execPath], { env: {}, encoding: 'utf8' });
  if (loaded.error) throw loaded.error;
  if (loaded.status !== 0) throw new Error('Could not load deployment configuration');
  const values = JSON.parse(loaded.stdout);
  for (const key of ['POSTGRES_PASSWORD', 'API_DATABASE_URL', 'API_PROVIDER_SECRET_MASTER_KEY',
    'BOOTSTRAP_ROOT_ACCOUNT', 'BOOTSTRAP_ROOT_PASSWORD']) {
    if (!values[key]) throw new Error(`Missing ${key} in deployment configuration`);
  }
  // PostgreSQL's official entrypoint consumes its password via *_FILE.
  const passwordFile = path.join(configRoot, 'postgres-password');
  fs.writeFileSync(passwordFile, values.POSTGRES_PASSWORD, { mode: 0o600 });
  fs.chmodSync(passwordFile, 0o600);
  return { created, file };
}

if (require.main === module) {
  try {
    const result = initialize();
    console.log(`${result.created ? 'Generated' : 'Reusing'} persistent configuration: ${result.file}`);
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}

module.exports = { initialize };
