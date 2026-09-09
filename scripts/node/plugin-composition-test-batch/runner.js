#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');
const { getAvailableParallelism, loadVerifyRuntimeConfig } = require('../testing/verify-runtime.js');
const { loadManifest, parseList, selectTests, passedExact, nodeTapResult, validateSources } = require('./selection.js');
const root = path.resolve(__dirname, '../../..');
const { data: manifest, filename: manifestPath } = loadManifest(root);
const output = path.resolve(root, manifest.outputDirectory || 'tmp/test-governance/2007');
const report = { schema: 1, scope: manifest.scope, batchMode: manifest.batchMode || 'full', startedAt: new Date().toISOString(), status: 'running', candidate: null, commands: [], node: [], targets: [], tests: [], blockers: [], identities: [], manifest };
fs.mkdirSync(output, { recursive: true });
let commandIndex = 0;
const env = { ...process.env };
// Keep real per-call integrity checks within the production observer budget even in debug tests.
// Only the SHA-256 dependency is optimized; the algorithm, full-byte checks and deadlines remain.
const cargoProfileArgs = manifest.scope === 'plugin-composition-2014' ? [
  '--config', 'profile.dev.package.sha2.opt-level=3',
  '--config', 'profile.test.package.sha2.opt-level=3',
] : [];
report.cargoProfileArgs = cargoProfileArgs;
delete env.RUST_TEST_THREADS; // Every Rust command selects exactly one test; the chain itself is serial.
function save() {
  report.finishedAt = new Date().toISOString();
  report.mapping = Object.fromEntries(['ac', 'auth'].flatMap(kind => [...new Set(manifest.required.flatMap(row => row[kind]))].sort().map(id => [id, manifest.required.filter(row => row[kind].includes(id)).map(row => ({ target: row.target, name: row.name, status: report.tests.find(test => test.target === row.target && test.name === row.name)?.status || 'not_run' }))])));
  fs.writeFileSync(path.join(output, 'report.json'), `${JSON.stringify(report, null, 2)}\n`);
  fs.writeFileSync(path.join(output, 'report.md'), `# 插件组合有限批次\n\n候选：${report.candidate || '未核验'}\n状态：${report.status}\n必需测试：${manifest.required.length}；已取得结果：${report.tests.length}\n\n${report.blockers.map(b => `- ${b}`).join('\n')}\n\n逐命令日志、测试状态与 AC/AUTH 映射见 report.json。未运行不计通过。\n`);
}
function run(label, command, args, options = {}) {
  const log = path.join(output, `${String(++commandIndex).padStart(4, '0')}-${label.replace(/[^a-z0-9_-]/giu, '_')}.log`);
  const startedAt = new Date().toISOString();
  const fd = fs.openSync(log, 'w');
  let result;
  try {
    result = spawnSync(command, args, { cwd: options.cwd || root, env, stdio: ['ignore', fd, fd], timeout: options.timeout || 30 * 60 * 1000, killSignal: 'SIGKILL', detached: true });
  } finally { fs.closeSync(fd); }
  if (result.signal || result.error) { try { process.kill(-result.pid, 'SIGKILL'); } catch {} }
  const entry = { label, command, args, cwd: options.cwd || root, startedAt, endedAt: new Date().toISOString(), exitCode: result.status, signal: result.signal, error: result.error?.message, log: path.relative(root, log) };
  report.commands.push(entry);
  save();
  const bytes = fs.statSync(log).size;
  if (bytes > 64 * 1024 * 1024) throw new Error(`${label}: log exceeds 64 MiB parse budget; see artifact`);
  return { code: result.status, text: fs.readFileSync(log, 'utf8') };
}
function requireCommand(label, command, args, options) {
  const result = run(label, command, args, options);
  if (result.code !== 0) throw new Error(`${label} failed (see command log)`);
  return result.text;
}
function hash(filename) { return crypto.createHash('sha256').update(fs.readFileSync(filename)).digest('hex'); }
function identity(filename) { report.identities.push({ path: path.relative(root, filename), sha256: hash(filename) }); }
try {
  const expected = process.env.PLUGIN_COMPOSITION_CANDIDATE_SHA;
  if (!/^[0-9a-f]{40}$/u.test(expected || '')) throw new Error('PLUGIN_COMPOSITION_CANDIDATE_SHA requires the frozen full SHA');
  report.candidate = requireCommand('candidate', 'git', ['rev-parse', 'HEAD']).trim();
  if (report.candidate !== expected) throw new Error('checkout SHA differs from frozen candidate');
  if (process.env.GITHUB_ACTIONS === 'true' && process.env.GITHUB_SHA !== expected) throw new Error('workflow dispatch definition SHA differs from frozen candidate; dispatch from candidate ref');
  if (requireCommand('tracked-clean', 'git', ['status', '--porcelain', '--untracked-files=no']).trim()) throw new Error('candidate tracked files are dirty');
  if (process.platform !== manifest.supportedPlatform) throw new Error('this finite batch requires Linux');
  if (env.CARGO_BUILD_TARGET) throw new Error('this native Linux batch does not accept CARGO_BUILD_TARGET');
  validateSources(root, manifest);
  for (const variable of ['DATABASE_URL', 'API_DATABASE_URL']) if (!env[variable]) throw new Error(`${variable} is required; no silent database fallback`);
  env.CARGO_TARGET_DIR = path.resolve(root, env.CARGO_TARGET_DIR || `tmp/quality-gate-cache/${manifest.scope}/target`);
  const availableParallelism = getAvailableParallelism();
  const runtimeConfig = loadVerifyRuntimeConfig({ repoRoot: root, env, availableParallelism });
  env.CARGO_BUILD_JOBS = String(runtimeConfig.backend.cargoJobs);
  report.resources = { availableParallelism, cargoJobs: runtimeConfig.backend.cargoJobs };

  identity(manifestPath);
  for (const file of manifest.identityFiles || []) identity(path.join(root, file));
  for (const target of manifest.targets) identity(path.join(root, target.packageRoot, 'Cargo.toml'));
  for (const file of ['api/Cargo.lock', 'api/crates/runtime-extension-sdk/Cargo.toml', 'api/crates/runtime-extension-sdk/src/_tests/managed_hook_worker.rs', 'api/crates/runtime-extension-sdk/src/_tests/managed_event_worker.rs', 'api/plugins/fixtures/acme.composition-a/manifest.yaml', 'api/plugins/fixtures/acme.composition-a/event-manifest.yaml', 'api/plugins/fixtures/acme.composition-b/manifest.yaml', 'api/plugins/fixtures/acme.composition-c/manifest.yaml']) identity(path.join(root, file));
  // Dependency-boundary regressions use locked offline cargo metadata; fetch this same lock first.
  requireCommand('locked-fetch', 'cargo', ['fetch', '--locked', '--manifest-path', 'api/Cargo.toml']);
  const workerFixtures = manifest.workerFixtures || [
    { example: 'managed_hook_worker', env: 'MANAGED_HOOK_WORKER_FIXTURE' },
    { example: 'managed_event_worker', env: 'MANAGED_EVENT_WORKER_FIXTURE' },
  ];
  for (const { example, env: variable } of workerFixtures) {
    try {
      requireCommand(`build-${example}`, 'cargo', ['build', ...cargoProfileArgs, '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'runtime-extension-sdk', '--example', example]);
      env[variable] = path.join(env.CARGO_TARGET_DIR, 'debug/examples', example);
      fs.accessSync(env[variable], fs.constants.X_OK);
      identity(env[variable]);
    } catch (error) { delete env[variable]; report.blockers.push(error.message); }
  }
  for (const node of manifest.node) {
    const result = run(node.id, process.execPath, node.args);
    const evidence = node.args.includes('--test') ? nodeTapResult(result.text, result.code)
      : { passed: result.code === 0, counts: null, reason: result.code === 0 ? null : `Node command exited with ${result.code}` };
    report.node.push({ id: node.id, ...evidence, commandIndex });
    if (!evidence.passed) report.blockers.push(`${node.id}: ${evidence.reason}`);
    save();
  }
  for (const target of manifest.targets) {
    const targetReport = { id: target.id, status: 'not_run', selected: [] };
    report.targets.push(targetReport);
    try {
      const targetArgs = target.target === 'lib' ? ['--lib'] : ['--test', target.target];
      const text = requireCommand(`compile-${target.id}`, 'cargo', ['test', ...cargoProfileArgs, '--locked', '--manifest-path', 'api/Cargo.toml', '-p', target.package, ...targetArgs, '--no-run', '--message-format=json']);
      const artifacts = text.split(/\r?\n/u).flatMap(line => { try { return [JSON.parse(line)]; } catch { return []; } }).filter(row => row.reason === 'compiler-artifact' && row.profile?.test && row.executable && row.target?.name === (target.target === 'lib' ? target.package.replaceAll('-', '_') : target.target));
      const binaries = [...new Set(artifacts.map(row => row.executable))];
      if (binaries.length !== 1) throw new Error(`${target.id}: expected one actual test artifact, got ${binaries.length}`);
      const binary = binaries[0];
      targetReport.artifact = { binary, packageId: artifacts[0].package_id, source: artifacts[0].target.src_path };
      const cwd = path.join(root, target.packageRoot); // Same working directory as cargo test.
      env.CARGO_MANIFEST_DIR = cwd;
      env.LD_LIBRARY_PATH = [path.dirname(binary), path.join(env.CARGO_TARGET_DIR, 'debug'), process.env.LD_LIBRARY_PATH].filter(Boolean).join(path.delimiter);
      const listed = parseList(requireCommand(`list-${target.id}`, binary, ['--list', '--format=terse'], { cwd }));
      targetReport.listedCount = listed.length;
      const selected = selectTests(target, listed, manifest.required, manifest);
      targetReport.selected = selected;
      targetReport.status = 'running';
      for (const name of selected) {
        const required = manifest.required.find(row => row.target === target.id && row.name === name);
        const missing = (required?.env || []).filter(variable => !env[variable]);
        if (missing.length) {
          report.tests.push({ target: target.id, name, status: 'blocked', missing });
          report.blockers.push(`${target.id}/${name}: missing ${missing.join(', ')}`);
          continue;
        }
        const result = run(`test-${target.id}`, binary, [name, '--exact', '--format=pretty', '--color=never', ...(manifest.showTestOutput ? ['--show-output'] : [])], { cwd, timeout: 5 * 60 * 1000 });
        const passed = passedExact(result.text, name, result.code);
        report.tests.push({ target: target.id, name, expected: 1, status: passed ? 'passed' : 'failed', commandIndex });
        if (!passed) report.blockers.push(`${target.id}/${name}: failed, ignored, timed out or wrong actual count`);
        save();
      }
      targetReport.status = report.tests.some(test => test.target === target.id && test.status !== 'passed') ? 'failed' : 'passed';
    } catch (error) { targetReport.status = 'blocked'; report.blockers.push(error.message); save(); }
  }
  for (const row of manifest.required) if (!report.tests.some(test => test.target === row.target && test.name === row.name && test.status === 'passed')) report.blockers.push(`required not passed: ${row.target}/${row.name}`);
  if (manifest.browserBinary && report.blockers.length === 0) {
    const { package: packageName, binary } = manifest.browserBinary;
    requireCommand('build-candidate-browser-api', 'cargo', ['build', ...cargoProfileArgs, '--locked', '--manifest-path', 'api/Cargo.toml', '-p', packageName, '--bin', binary]);
    const source = path.join(env.CARGO_TARGET_DIR, 'debug', binary);
    const directory = path.join(output, 'candidate-api');
    fs.mkdirSync(directory, { recursive: true });
    const destination = path.join(directory, binary);
    fs.copyFileSync(source, destination);
    fs.chmodSync(destination, 0o755);
    const binaryIdentity = { candidate: report.candidate, sha256: hash(destination), binary };
    fs.writeFileSync(`${destination}.identity.json`, `${JSON.stringify(binaryIdentity, null, 2)}\n`);
    report.browserBinary = binaryIdentity;
  }
  report.status = report.blockers.length ? 'failed' : 'passed';
} catch (error) { report.status = 'failed'; report.blockers.push(error.stack || error.message); }
finally { save(); process.stdout.write(`${manifest.scope}: ${report.status}; ${path.relative(root, output)}/report.json\n`); process.exitCode = report.status === 'passed' ? 0 : 1; }
