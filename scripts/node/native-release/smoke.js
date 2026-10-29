#!/usr/bin/env node
'use strict';
// Public HTTP + isolated database lifecycle smoke; never uses the developer database.
const fs = require('node:fs');
const path = require('node:path');
const net = require('node:net');
const assert = require('node:assert/strict');
const { spawn, spawnSync } = require('node:child_process');
const { once } = require('node:events');
const { setTimeout: delay } = require('node:timers/promises');
const { openTemporaryOwnerSession } = require('../page-debug/auth.js');
const { readEnv, writeEnv } = require('./test-env.js');

async function freePort() {
  const server = net.createServer();
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  return port;
}
async function smoke(stage, evidence) {
  assert.notEqual(process.platform, 'win32', 'Windows graceful console shutdown requires a native/manual test');
  stage = path.resolve(stage);
  evidence = path.resolve(evidence);
  fs.mkdirSync(evidence, { recursive: true, mode: 0o700 });
  const data = path.join(evidence, 'data');
  assert.ok(!fs.existsSync(data), 'Use a fresh evidence directory');
  const httpPort = await freePort();
  let pgPort = await freePort();
  while (pgPort === httpPort) pgPort = await freePort();
  const env = { ...process.env };
  writeEnv(stage, { FLOWBASE_DATA_DIR: data, API_SERVER_ADDR: `127.0.0.1:${httpPort}`, API_DATABASE_URL: `postgres://postgres:__GENERATE__@127.0.0.1:${pgPort}/1flowbase`, API_PROVIDER_SECRET_MASTER_KEY: '__GENERATE__', API_AUTH_PUBLIC_ORIGIN: `http://127.0.0.1:${httpPort}`, API_ALLOWED_ORIGINS: `http://127.0.0.1:${httpPort}`, FLOWBASE_OPEN_BROWSER: 'false', API_BUSINESS_FILE_LOCAL_ROOT: path.join(data, 'api/storage'), API_PROVIDER_INSTALL_ROOT: path.join(data, 'api/plugins'), API_HOST_EXTENSION_DROPIN_ROOT: path.join(data, 'api/plugins/host-extension/dropins'), API_MCP_TEMPLATE_LIBRARY_ROOT: path.join(data, 'api/storage/extension-center/mcp'), API_SYSTEM_BACKUP_REPOSITORY_ROOT: path.join(data, 'backups') });
  const executable = path.join(stage, '1flowbase');
  const origin = `http://127.0.0.1:${httpPort}`;
  let child;
  let exited;
  let settings;
  function sql(query) {
    const result = spawnSync(path.join(stage, 'postgres/bin/psql'), ['-X', '-w', '-h', '127.0.0.1', '-p', String(pgPort), '-U', 'postgres', '-d', '1flowbase', '-v', 'ON_ERROR_STOP=1', '-Atc', query], { env: { ...process.env, PGPASSWORD: settings.postgres_password }, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  }
  async function stop() {
    if (!child) return;
    if (child.exitCode === null && child.signalCode === null) child.kill('SIGINT');
    const result = await Promise.race([exited, delay(60000, null, { ref: false })]);
    assert.ok(result, 'Graceful shutdown timed out');
    assert.equal(result[0], 0, `Process exit: ${result}`);
    child = null;
    assert.ok(!fs.existsSync(path.join(data, 'postgres/postmaster.pid')), 'PostgreSQL must stop with the API');
  }
  try {
    for (let iteration = 0; iteration < 2; iteration++) {
      const log = fs.openSync(path.join(evidence, `startup-${iteration}.log`), 'w', 0o600);
      child = spawn(executable, [], { env, cwd: stage, stdio: ['ignore', log, log] });
      fs.closeSync(log);
      exited = once(child, 'exit');
      const deadline = Date.now() + 180000;
      let ready = false;
      while (Date.now() < deadline) {
        if (child.exitCode !== null || child.signalCode !== null) throw new Error(`Native process exited; inspect startup-${iteration}.log`);
        try {
          const response = await fetch(origin + '/health', { signal: AbortSignal.timeout(1500) });
          if (response.ok) { ready = true; break; }
        } catch {}
        await delay(300);
      }
      assert.ok(ready, 'Native HTTP readiness timed out');
      const index = await fetch(origin + '/');
      assert.equal(index.status, 200);
      assert.match(await index.text(), /<html/i);
      const deep = await fetch(origin + '/native-smoke/deep-link', { headers: { accept: 'text/html' } });
      assert.equal(deep.status, 200);
      assert.equal((await fetch(origin + '/api/native-smoke-missing', { headers: { accept: 'text/html' } })).status, 404);
      const values = readEnv(stage);
      const current = { postgres_password: decodeURIComponent(new URL(values.API_DATABASE_URL).password), root_password: values.BOOTSTRAP_ROOT_PASSWORD, root_account: values.BOOTSTRAP_ROOT_ACCOUNT, master_key: values.API_PROVIDER_SECRET_MASTER_KEY };
      if (iteration === 0) {
        settings = current;
        const session = await openTemporaryOwnerSession({ apiBaseUrl: origin, account: settings.root_account, password: settings.root_password });
        try {
          const me = await fetch(origin + '/api/console/me', { headers: { cookie: session.cookie }, signal: AbortSignal.timeout(10000) });
          assert.equal(me.status, 200, 'Authenticated console route must remain reachable');
        } finally { await session.dispose(); }
        sql('CREATE TABLE native_release_smoke (value text); INSERT INTO native_release_smoke VALUES (\'persisted\')');
        const backup = path.join(evidence, 'sentinel.dump');
        const pgEnv = { ...process.env, PGPASSWORD: settings.postgres_password };
        const dump = spawnSync(path.join(stage, 'postgres/bin/pg_dump'), ['-w', '-h', '127.0.0.1', '-p', String(pgPort), '-U', 'postgres', '-d', '1flowbase', '-Fc', '-t', 'native_release_smoke', '-f', backup], { env: pgEnv, encoding: 'utf8' });
        assert.equal(dump.status, 0, dump.stderr);
        sql('CREATE DATABASE native_release_smoke_restore');
        const restore = spawnSync(path.join(stage, 'postgres/bin/pg_restore'), ['-w', '-h', '127.0.0.1', '-p', String(pgPort), '-U', 'postgres', '-d', 'native_release_smoke_restore', '--exit-on-error', backup], { env: pgEnv, encoding: 'utf8' });
        assert.equal(restore.status, 0, restore.stderr);
        const restored = spawnSync(path.join(stage, 'postgres/bin/psql'), ['-X', '-w', '-h', '127.0.0.1', '-p', String(pgPort), '-U', 'postgres', '-d', 'native_release_smoke_restore', '-Atc', 'SELECT value FROM native_release_smoke'], { env: pgEnv, encoding: 'utf8' });
        assert.equal(restored.status, 0, restored.stderr);
        assert.equal(restored.stdout.trim(), 'persisted');
        sql('DROP DATABASE native_release_smoke_restore');
        const duplicate = spawnSync(executable, [], { env, cwd: stage, encoding: 'utf8', timeout: 15000 });
        assert.notEqual(duplicate.status, 0);
        assert.match(duplicate.stderr, /already running/);
        const savedEnv = fs.readFileSync(path.join(stage, '.env'));
        writeEnv(stage, { FLOWBASE_DATA_DIR: path.join(evidence, 'conflict'), API_SERVER_ADDR: `127.0.0.1:${await freePort()}` });
        const conflict = spawnSync(executable, [], { env, cwd: stage, encoding: 'utf8', timeout: 15000 });
        fs.writeFileSync(path.join(stage, '.env'), savedEnv);
        assert.notEqual(conflict.status, 0);
        assert.match(conflict.stderr, /PostgreSQL port .* occupied/);
        assert.ok(!fs.existsSync(path.join(evidence, 'conflict/postgres')));
      } else {
        assert.deepEqual(current, settings, 'Credentials must survive restarts');
        assert.equal(sql('SELECT value FROM native_release_smoke'), 'persisted');
        sql('DROP TABLE native_release_smoke');
      }
      await stop();
    }
    const result = { platform: process.platform, arch: process.arch, http_port: httpPort, postgres_port: pgPort, passed: ['first-start', 'root-login-and-session-cleanup', 'pg-dump-restore', 'static-index', 'spa-deep-link', 'api-404', 'instance-lock', 'postgres-port-conflict', 'restart-persistence', 'stable-secrets', 'graceful-stop'], archive_defaults: { http_port: 7800, postgres_port: 35432 } };
    fs.writeFileSync(path.join(evidence, 'result.json'), JSON.stringify(result, null, 2) + '\n');
    console.log(JSON.stringify(result));
  } finally {
    if (child && child.exitCode === null && child.signalCode === null) {
      child.kill('SIGINT');
      const done = await Promise.race([exited, delay(10000, null, { ref: false })]);
      if (!done) { child.kill('SIGKILL'); await exited; }
      spawnSync(path.join(stage, 'postgres/bin/pg_ctl'), ['stop', '-w', '-t', '20', '-m', 'fast', '-D', path.join(data, 'postgres')], { stdio: 'ignore' });
    }
  }
}
if (require.main === module) smoke(process.argv[2], process.argv[3]).catch(error => { console.error(error.message); process.exitCode = 1; });
module.exports = { smoke };
