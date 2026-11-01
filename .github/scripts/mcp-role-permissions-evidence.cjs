'use strict';
// One-task verification adapter; product source and tests remain frozen.
const fs = require('node:fs');
const path = require('node:path');
const cp = require('node:child_process');
const {requireExecutedCargoTests} = require('../../scripts/node/verify/cargo-test-results.js');
const source = '97702c7932ec2bc81801258f395d707ce2e42159';
const out = path.resolve('tmp/test-governance/mcp-role-permissions-ci');
fs.mkdirSync(out, {recursive: true});
const git = (...args) => cp.execFileSync('git', args, {encoding: 'utf8'}).trim();
const head = git('rev-parse', 'HEAD');
const apiTree = git('rev-parse', 'HEAD:api');
if (apiTree !== git('rev-parse', source + ':api')) throw Error('Frozen API tree mismatch');
const diff = git('diff', '--name-only', source, head);
const allowed = new Set(['.github/workflows/mcp-role-permissions-evidence.yml', '.github/scripts/mcp-role-permissions-evidence.cjs']);
if (diff.split('\n').filter(Boolean).some(file => !allowed.has(file))) throw Error('Non-CI source changed');
const jobs = cp.execFileSync(process.execPath, ['scripts/node/testing/verify-runtime.js', 'cargo-jobs'], {encoding: 'utf8'}).trim();
if (!/^[1-9]\d*$/.test(jobs)) throw Error('Invalid repository helper job count');
const filters = [
  'mcp_role_permission_',
  'ac_001_ac_005_ac_010_interface_catalog_covers_compiled_routes_and_media_boundaries',
  'role_routes',
  'console_navigation_route_',
  'runtime_mcp_write_',
  'ac_003_protocol_uses_api_key_actor_workspace_and_rejects_cross_workspace_instance'
];
const receipt = {source, head, apiTree, jobs: Number(jobs), startedAt: new Date().toISOString(), results: []};
const save = () => fs.writeFileSync(path.join(out, 'results.json'), JSON.stringify(receipt, null, 2) + '\n');
save();
for (const filter of filters) {
  const args = ['--config', 'profile.test.package.api-server.debug=0', '--config', 'profile.test.package.api-server.codegen-units=256', 'test', '--locked', '--manifest-path', 'api/Cargo.toml', '--package', 'api-server', '--lib', '--jobs', jobs, filter, '--', '--test-threads=' + jobs];
  const logfile = path.join(out, filter + '.log');
  const fd = fs.openSync(logfile, 'w');
  const start = Date.now();
  const result = cp.spawnSync('cargo', args, {stdio: ['ignore', fd, fd], timeout: 50 * 60 * 1000});
  fs.closeSync(fd);
  const log = fs.readFileSync(logfile, 'utf8');
  let counts = null, error = null;
  try {counts = requireExecutedCargoTests(log, [filter]);} catch (e) {error = e.message;}
  const item = {filter, command: ['cargo', ...args], code: result.status, signal: result.signal, seconds: (Date.now() - start) / 1000, counts, error: error || result.error?.message || null};
  receipt.results.push(item); save();
  console.log(JSON.stringify(item));
  if (item.code !== 0 || item.error) {
    console.log(log.slice(-10000));
    process.exitCode = 1;
    // Never repeat failed compilation under another filter.
    if (!counts) break;
  }
}
receipt.finishedAt = new Date().toISOString();
receipt.pass = receipt.results.length === filters.length && receipt.results.every(r => r.code === 0 && !r.error && r.counts?.passedCount > 0 && r.counts.failedCount === 0);
save();
if (!receipt.pass) process.exitCode = 1;
