#!/usr/bin/env node
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const net = require('node:net');
const assert = require('node:assert/strict');
const { once } = require('node:events');
const { spawnSync } = require('node:child_process');
const { readEnv, writeEnv } = require('./test-env.js');

async function freePort() {
  const socket = net.createServer().listen(0, '127.0.0.1');
  await once(socket, 'listening');
  const port = socket.address().port;
  await new Promise(resolve => socket.close(resolve));
  return port;
}
async function smoke(stage, evidence) {
  stage = path.resolve(stage); evidence = path.resolve(evidence);
  assert.ok(!fs.existsSync(evidence), 'Use a fresh evidence directory');
  fs.mkdirSync(evidence, { recursive: true });
  const target = path.join(evidence, 'old installation');
  const candidate = path.join(evidence, 'new package');
  const suffix = process.platform === 'win32' ? '.exe' : '';
  const executable = `1flowbase${suffix}`;
  for (const root of [target, candidate]) {
    fs.cpSync(stage, root, { recursive: true, filter: source => source !== path.join(stage, 'data') });
    assert.ok(!fs.existsSync(path.join(root, 'data')));
    fs.copyFileSync(path.join(root, '.env.example'), path.join(root, '.env'));
  }
  const http = await freePort(); let pg = await freePort(); while (pg === http) pg = await freePort();
  writeEnv(target, { API_SERVER_ADDR: `127.0.0.1:${http}`, API_DATABASE_URL: `postgres://postgres:__GENERATE__@127.0.0.1:${pg}/1flowbase`, API_AUTH_PUBLIC_ORIGIN: `http://127.0.0.1:${http}`, API_ALLOWED_ORIGINS: `http://127.0.0.1:${http}`, FLOWBASE_OPEN_BROWSER: 'false', BOOTSTRAP_ROOT_ACCOUNT: 'upgrade-owner', BOOTSTRAP_ROOT_PASSWORD: 'Upgrade-fixture123' });
  let number = 0;
  function run(root, args, succeeds = true) {
    const log = path.join(evidence, `command-${++number}.log`);
    const fd = fs.openSync(log, 'w');
    const result = spawnSync(path.join(root, executable), args, { cwd: evidence, stdio: ['ignore', fd, fd], timeout: 300000, env: process.env });
    fs.closeSync(fd);
    assert.equal(result.error, undefined, `${log}: ${result.error}`);
    if (succeeds) assert.equal(result.status, 0, fs.readFileSync(log, 'utf8').slice(-8000));
    else assert.notEqual(result.status, 0, `Expected refusal: ${args}`);
  }
  function pgCommand(name, args, env = {}) {
    const result = spawnSync(path.join(target, `postgres/bin/${name}${suffix}`), args, { cwd: target, env: { ...process.env, ...env }, encoding: 'utf8', timeout: 90000 });
    assert.equal(result.status, 0, result.stderr || result.error?.message);
    return result.stdout.trim();
  }
  const cluster = path.join(target, 'data/postgres');
  function withDatabase(callback) {
    const values = readEnv(target);
    const password = decodeURIComponent(new URL(values.API_DATABASE_URL).password);
    pgCommand('pg_ctl', ['start','-w','-t','60','-D',cluster,'-l',path.join(evidence,'postgres.log'),'-o',`-p ${pg} -h 127.0.0.1 -c unix_socket_directories=`]);
    try {
      const sql = query => pgCommand('psql', ['-X','-w','-h','127.0.0.1','-p',String(pg),'-U','postgres','-d','1flowbase','-v','ON_ERROR_STOP=1','-Atc',query], { PGPASSWORD: password });
      callback(sql);
    } finally { pgCommand('pg_ctl', ['stop','-w','-t','60','-m','fast','-D',cluster]); }
  }
  try {
    run(target, ['--check-startup']);
    const environment = fs.readFileSync(path.join(target,'.env'),'utf8');
    withDatabase(sql => {
      assert.equal(sql("SELECT count(*) FROM users WHERE account='upgrade-owner'"), '1');
      sql("CREATE TABLE native_upgrade_probe (value text); INSERT INTO native_upgrade_probe VALUES ('before')");
    });
    fs.writeFileSync(path.join(target,'data/api/storage/upgrade-file.txt'), 'user file');
    fs.writeFileSync(path.join(target,'data/api/plugins/upgrade-plugin.txt'), 'installed plugin');
    const originalManifest = JSON.parse(fs.readFileSync(path.join(target,'native-distribution.json')));
    originalManifest.version = '0.0.0';
    originalManifest.build_id = `upgrade-baseline-fixture-${originalManifest.build_id}`;
    fs.writeFileSync(path.join(target,'README.md'),'previous release fixture\n');
    originalManifest.files['README.md'] = crypto.createHash('sha256').update(fs.readFileSync(path.join(target,'README.md'))).digest('hex');
    fs.writeFileSync(path.join(target,'native-distribution.json'),JSON.stringify(originalManifest));
    // Real process/cluster refusal must leave the database running and readable.
    withDatabase(sql => { run(candidate,['--upgrade',target],false); assert.equal(sql('SELECT value FROM native_upgrade_probe'),'before'); });
    run(candidate,['--upgrade',target]);
    assert.equal(fs.readFileSync(path.join(target,'.env'),'utf8'),environment);
    assert.equal(fs.readFileSync(path.join(target,'data/api/storage/upgrade-file.txt'),'utf8'),'user file');
    assert.equal(fs.readFileSync(path.join(target,'data/api/plugins/upgrade-plugin.txt'),'utf8'),'installed plugin');
    assert.equal(fs.readFileSync(path.join(target,'README.md'),'utf8'),fs.readFileSync(path.join(candidate,'README.md'),'utf8'));
    const backupRoot = path.join(evidence,'.old installation-upgrade-backups');
    const firstBackup = path.join(backupRoot,fs.readdirSync(backupRoot)[0]);
    withDatabase(sql => { assert.equal(sql('SELECT value FROM native_upgrade_probe'),'before'); sql("UPDATE native_upgrade_probe SET value='after-upgrade'"); });
    run(candidate,['--restore',target,firstBackup]);
    withDatabase(sql => assert.equal(sql('SELECT value FROM native_upgrade_probe'),'before'));
    assert.equal(fs.readFileSync(path.join(target,'README.md'),'utf8'),'previous release fixture\n');
    // Valid checksums do not imply a bootable application. Corrupt the plugin-set document
    // and update its checksum, exercising post-replacement initialization failure.
    const brokenFile = 'api/plugins/sets/default.yaml';
    fs.writeFileSync(path.join(candidate,brokenFile),'[ invalid plugin set');
    const brokenManifest = JSON.parse(fs.readFileSync(path.join(candidate,'native-distribution.json')));
    brokenManifest.build_id += '-startup-failure-fixture';
    brokenManifest.files[brokenFile] = crypto.createHash('sha256').update(fs.readFileSync(path.join(candidate,brokenFile))).digest('hex');
    fs.writeFileSync(path.join(candidate,'native-distribution.json'),JSON.stringify(brokenManifest));
    run(candidate,['--upgrade',target],false);
    const state = JSON.parse(fs.readFileSync(path.join(target,'.upgrade-state.json'),'utf8'));
    assert.equal(state.phase,'failed');
    run(target,[],false);
    // Simulate persisted writes from a partial schema migration, then verify physical restore.
    withDatabase(sql => sql("UPDATE native_upgrade_probe SET value='partial-migration'"));
    run(candidate,['--restore',target,state.backup]);
    withDatabase(sql => assert.equal(sql('SELECT value FROM native_upgrade_probe'),'before'));
    run(target,['--check-startup']);
    assert.equal(fs.readFileSync(path.join(target,'.env'),'utf8'),environment);
    const result = { platform:process.platform, arch:process.arch, baseline:'controlled preceding-release layout using the candidate executable', passed:['editable-env','running-cluster-refusal','upgrade-config-data-files-plugins','cold-database-restore','startup-failure-blocks-launch','failed-upgrade-recovery','restored-startup'] };
    fs.writeFileSync(path.join(evidence,'result.json'),JSON.stringify(result,null,2));
    console.log(JSON.stringify(result));
  } finally {
    if (fs.existsSync(path.join(cluster,'postmaster.pid'))) spawnSync(path.join(target,`postgres/bin/pg_ctl${suffix}`),['stop','-w','-t','60','-m','fast','-D',cluster],{stdio:'ignore'});
  }
}
if(require.main===module)smoke(process.argv[2],process.argv[3]).catch(error=>{console.error(error);process.exitCode=1;});
module.exports={smoke};
