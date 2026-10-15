// Experimental dependency comparison. Product gates and their assertions stay unchanged.
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const { spawn, execFileSync } = require('node:child_process');

const root = path.resolve(process.env.COMPARISON_ROOT || path.join(__dirname, '../../..'));
const lane = process.env.COMPARISON_LANE;
const mode = process.env.COMPARISON_MODE;
const variant = process.env.COMPARISON_VARIANT;
const sample = process.env.COMPARISON_SAMPLE || 'single';
const targetDir = process.env.CARGO_TARGET_DIR;
const out = path.resolve(process.env.COMPARISON_OUTPUT_DIR || path.join(root, 'tmp/test-governance/dependency-comparison'), `${variant}-${lane}-${sample}`);
const { BACKEND_SHARDS, BACKEND_CI_TEST_SHARDS } = require('../verify/backend-targets.js');
const validLanes = new Set([
  ...BACKEND_SHARDS.map(({ key }) => `compile-${key}`),
  ...BACKEND_CI_TEST_SHARDS.map(({ key }) => `test-${key}`),
]);
if (!['probe', 'correctness', 'timing'].includes(mode) || !validLanes.has(lane)
    || !['baseline', 'compatible', 'latest'].includes(variant)
    || !/^[a-zA-Z0-9.-]+$/.test(sample)) throw new Error('Invalid explicit comparison inputs');
const gate = (...args) => ['node', 'scripts/node/verify-backend.js', ...args];
const commands = lane.startsWith('compile-')
  ? (mode === 'correctness' ? ['check', 'clippy'] : ['check']).map((kind) => gate(kind, lane.slice(8)))
  : [gate('test', lane.slice(5))];
if (process.argv.includes('--plan')) {
  console.log(JSON.stringify({ root, lane, mode, variant, commands }));
} else {
  main().catch((error) => { console.error(error); process.exitCode = 1; });
}

async function main() {
  if (process.env.GITHUB_ACTIONS !== 'true') throw new Error('Heavy execution belongs in GitHub Actions');
  if (!targetDir || fs.existsSync(targetDir)) throw new Error('A fresh absolute CARGO_TARGET_DIR is required');
  if (!path.isAbsolute(targetDir)) throw new Error('Target directory must be absolute');
  fs.mkdirSync(out, { recursive: true });
  const capture = (command, args) => execFileSync(command, args, { cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }).trim();
  const lockHash = () => crypto.createHash('sha256').update(fs.readFileSync(path.join(root, 'api/Cargo.lock'))).digest('hex');
  const receipt = {
    lane, mode, variant, sample, sha: capture('git', ['rev-parse', 'HEAD']),
    baseSha: '6345210d59e95ed9ec34baf2eec5ca39e417dde5',
    rustc: capture('rustc', ['--version', '--verbose']), cargo: capture('cargo', ['--version']),
    cpu: os.cpus()[0].model, cpus: os.availableParallelism(), memoryBytes: os.totalmem(),
    runnerImage: process.env.ImageVersion, runnerOS: process.env.ImageOS,
    runId: process.env.GITHUB_RUN_ID, attempt: process.env.GITHUB_RUN_ATTEMPT,
    lockHashBefore: lockHash(), status: 'running', steps: [],
    cache: 'empty target; no restored caches; dependency fetch excluded from gate timing',
    warmPolicy: mode === 'timing' ? 'only immediate unchanged cargo check repeat' : 'no warm test repeat',
  };
  const save = () => fs.writeFileSync(path.join(out, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  save();
  let active;
  process.on('SIGTERM', () => {
    receipt.status = 'interrupted'; save();
    if (active) { try { process.kill(-active.pid, 'SIGTERM'); } catch {} }
    process.exitCode = 143;
  });
  async function run(label, command, offline = true) {
    const phaseDir = path.join(out, label);
    fs.mkdirSync(phaseDir, { recursive: true });
    const logPath = path.join(phaseDir, 'command.log');
    const fd = fs.openSync(logPath, 'w');
    const started = performance.now();
    let timedOut = false;
    let result;
    const deadline = Number(process.env.COMPARISON_PHASE_MINUTES || '55') * 60 * 1000;
    const child = spawn(command[0], command.slice(1), {
      cwd: root, detached: true,
      env: { ...process.env, CARGO_NET_OFFLINE: String(offline), ONEFLOWBASE_WARNING_OUTPUT_DIR: phaseDir },
      stdio: ['ignore', fd, fd],
    });
    active = child;
    let killTimer;
    const timeout = setTimeout(() => {
      timedOut = true;
      try { process.kill(-child.pid, 'SIGTERM'); } catch {}
      killTimer = setTimeout(() => { try { process.kill(-child.pid, 'SIGKILL'); } catch {} }, 5000);
    }, deadline);
    const heartbeat = setInterval(() => console.log(`${label}: running ${(performance.now() - started) / 1000 | 0}s`), 60000);
    try {
      result = await new Promise((resolve) => {
        child.once('error', (error) => resolve({ exitCode: null, error: error.message }));
        child.once('exit', (exitCode, signal) => resolve({ exitCode, signal }));
      });
    } finally {
      clearTimeout(timeout); clearTimeout(killTimer); clearInterval(heartbeat); fs.closeSync(fd); active = undefined;
    }
    const step = { label, command, ...result, timedOut, seconds: (performance.now() - started) / 1000 };
    receipt.steps.push(step); save(); console.log(JSON.stringify(step));
    if (result.exitCode !== 0) console.log(fs.readFileSync(logPath, 'utf8').slice(-5000));
    return !timedOut && result.exitCode === 0;
  }
  let failed = false;
  const fetched = await run('dependency-fetch', ['cargo', 'fetch', '--locked', '--manifest-path', 'api/Cargo.toml'], false);
  if (!fetched) failed = true;
  if (fetched) {
    const metadataPath = path.join(out, 'metadata.json');
    fs.writeFileSync(metadataPath, capture('cargo', ['metadata', '--locked', '--offline', '--format-version', '1', '--manifest-path', 'api/Cargo.toml']));
    const metadata = JSON.parse(fs.readFileSync(metadataPath, 'utf8'));
    const packages = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]));
    const direct = new Map();
    for (const node of metadata.resolve.nodes.filter((node) => metadata.workspace_members.includes(node.id))) {
      for (const dep of node.deps) {
        const pkg = packages.get(dep.pkg);
        if (!pkg.source?.startsWith('registry+')) continue;
        const versions = direct.get(pkg.name) || new Set(); versions.add(pkg.version); direct.set(pkg.name, versions);
      }
    }
    const registry = metadata.packages.filter((pkg) => pkg.source?.startsWith('registry+'));
    const names = new Set(registry.map((pkg) => pkg.name));
    receipt.dependencies = { direct: Object.fromEntries([...direct].sort().map(([name, versions]) => [name, [...versions].sort()])), registryPackages: registry.length, duplicateVersionCount: registry.length - names.size };
    save();
    if (lane === 'test-runtime-storage-fast' || lane.startsWith('test-api-server-')) {
      const examples = ['managed_hook_worker', 'managed_event_worker', 'notification_worker', 'notification_authorization_worker'];
      const built = await run('worker-fixtures', ['cargo', 'build', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'runtime-extension-sdk', ...examples.flatMap((name) => ['--example', name])]);
      if (!built) failed = true;
      for (const example of examples) {
        process.env[`${example.toUpperCase()}_FIXTURE`] = path.join(targetDir, 'debug/examples', example);
        if (built) capture('strip', ['--strip-debug', process.env[`${example.toUpperCase()}_FIXTURE`]]);
      }
    }
    for (const command of commands) {
      const kind = command[2]; const passed = await run(`cold-${kind}`, command);
      if (!passed) failed = true;
      if (mode === 'timing' && passed && !await run(`warm-${kind}`, command)) failed = true;
      if (!passed && (kind === 'check' || receipt.steps.at(-1).timedOut)) break;
    }
  }
  receipt.lockHashAfter = lockHash();
  if (receipt.lockHashBefore !== receipt.lockHashAfter) failed = true;
  receipt.status = receipt.steps.some((step) => step.timedOut) ? 'timed_out' : failed ? 'failed' : 'passed';
  save();
  fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, [
    `### ${variant} / ${lane} / ${sample}`, '', `Source: ${receipt.sha}; CPU: ${receipt.cpu}`, '',
    '| Phase | Seconds | Exit |', '|---|---:|---:|',
    ...receipt.steps.map((step) => `| ${step.label} | ${step.seconds.toFixed(2)} | ${step.timedOut ? 'timeout' : step.exitCode ?? step.signal} |`), '',
    'Gate time excludes fetch/setup. Clippy reuses prior check artifacts. Fixture builds prewarm some test dependencies.', '',
  ].join('\n'));
  process.exitCode = failed ? 1 : 0;
}
