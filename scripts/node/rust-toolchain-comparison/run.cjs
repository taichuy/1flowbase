// Isolated CI experiment; invokes existing gates without changing their policy.
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const { spawn, execFileSync } = require('node:child_process');

const root = path.resolve(__dirname, '../../..');
const lane = process.env.COMPARISON_LANE;
const toolchain = process.env.RUSTUP_TOOLCHAIN;
const gate = (...args) => ['node', 'scripts/node/verify-backend.js', ...args];
const shards = require('../verify/backend-targets.js');
const validLanes = new Set([
  'fmt', ...shards.BACKEND_SHARDS.map(({ key }) => `compile-${key}`),
  ...shards.BACKEND_CI_TEST_SHARDS.map(({ key }) => `test-${key}`),
]);
if (!validLanes.has(lane) || !['1.97.1', '1.99.0'].includes(toolchain)) {
  throw new Error('Explicit supported COMPARISON_LANE and RUSTUP_TOOLCHAIN required');
}
let commands = lane === 'fmt' ? [gate('fmt')]
  : lane.startsWith('compile-')
    ? ['check', 'clippy'].map((kind) => gate(kind, lane.slice('compile-'.length)))
    : [gate('test', lane.slice('test-'.length))];
if (process.argv.includes('--timing-only')) {
  if (!lane.startsWith('compile-')) throw new Error('Paired timing supports compile lanes');
  commands = commands.filter((command) => command[2] === 'check');
}
if (process.argv.includes('--plan')) {
  console.log(JSON.stringify({ lane, toolchain, commands }));
} else {
  main().catch((error) => { console.error(error); process.exitCode = 1; });
}

async function main() {
  if (process.env.GITHUB_ACTIONS !== 'true') throw new Error('Run heavy comparisons in GitHub Actions');
  const sample = process.env.COMPARISON_SAMPLE || 'rust-comparison';
  if (!/^[a-zA-Z0-9.-]+$/.test(sample)) throw new Error('Invalid sample name');
  const out = path.join(root, 'tmp/test-governance', sample);
  fs.mkdirSync(out, { recursive: true });
  const capture = (command, args) => execFileSync(command, args, { cwd: root, encoding: 'utf8' }).trim();
  const lockHash = () => crypto.createHash('sha256').update(fs.readFileSync(path.join(root, 'api/Cargo.lock'))).digest('hex');
  const receipt = {
    lane, toolchain, sample, sha: capture('git', ['rev-parse', 'HEAD']),
    baseSha: '6345210d59e95ed9ec34baf2eec5ca39e417dde5',
    rustc: capture('rustc', ['--version', '--verbose']), cargo: capture('cargo', ['--version']),
    runnerImage: process.env.ImageVersion, runnerOS: process.env.ImageOS,
    cpu: os.cpus()[0].model, cpus: os.availableParallelism(), memoryBytes: os.totalmem(),
    lockHashBefore: lockHash(), runId: process.env.GITHUB_RUN_ID,
    cache: 'empty target; no restored caches; dependencies fetched before cold gate',
    warmPolicy: 'immediate unchanged rerun of each successful gate, before the next gate',
    steps: [],
  };
  const save = () => fs.writeFileSync(path.join(out, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  save();
  if (fs.existsSync(process.env.CARGO_TARGET_DIR)) throw new Error('Cold target must not exist');
  async function run(label, command, offline = true) {
    const phaseDir = path.join(out, label);
    fs.mkdirSync(phaseDir, { recursive: true });
    const logPath = path.join(phaseDir, 'command.log');
    const log = fs.openSync(logPath, 'w');
    const started = performance.now();
    let code;
    let signal;
    try {
      [code, signal] = await new Promise((resolve, reject) => {
        const child = spawn(command[0], command.slice(1), {
          cwd: root,
          env: { ...process.env, CARGO_NET_OFFLINE: String(offline), ONEFLOWBASE_WARNING_OUTPUT_DIR: phaseDir },
          stdio: ['ignore', log, log],
        });
        child.on('error', reject);
        child.on('exit', (exitCode, exitSignal) => resolve([exitCode, exitSignal]));
      });
    } finally { fs.closeSync(log); }
    const result = { label, command, exitCode: code, signal, seconds: (performance.now() - started) / 1000 };
    receipt.steps.push(result);
    save();
    console.log(JSON.stringify(result));
    if (code !== 0) {
      const text = fs.readFileSync(logPath, 'utf8');
      console.log(text.slice(-5000));
    }
    return code === 0;
  }
  let failed = false;
  const fetched = await run('dependency-fetch', ['cargo', 'fetch', '--locked', '--manifest-path', 'api/Cargo.toml'], false);
  if (!fetched) failed = true;
  if (fetched) {
    if (lane === 'test-runtime-storage-fast' || lane.startsWith('test-api-server-')) {
      const examples = ['managed_hook_worker', 'managed_event_worker', 'notification_worker', 'notification_authorization_worker'];
      const built = await run('worker-fixtures', [
        'cargo', 'build', '--locked', '--manifest-path', 'api/Cargo.toml', '-p', 'runtime-extension-sdk',
        ...examples.flatMap((name) => ['--example', name]),
      ]);
      if (!built) failed = true;
      for (const example of examples) {
        process.env[`${example.toUpperCase()}_FIXTURE`] = path.join(process.env.CARGO_TARGET_DIR, 'debug/examples', example);
        if (built) capture('strip', ['--strip-debug', process.env[`${example.toUpperCase()}_FIXTURE`]]);
      }
    }
    for (const command of commands) {
      const kind = command[2];
      const passed = await run(`cold-${kind}`, command);
      if (!passed) failed = true;
      // Failed cold gates have no comparable warm timing; retain their real error.
      if (passed && !await run(`warm-${kind}`, command)) failed = true;
    }
  }
  receipt.lockHashAfter = lockHash();
  if (receipt.lockHashBefore !== receipt.lockHashAfter) failed = true;
  receipt.status = failed ? 'failed' : 'passed';
  save();
  const summary = [
    `### ${toolchain} / ${lane}`, '',
    `Source: ${receipt.sha}; CPU: ${receipt.cpu}; CPUs: ${receipt.cpus}`, '',
    '| Phase | Seconds | Exit |', '|---|---:|---:|',
    ...receipt.steps.map((step) => `| ${step.label} | ${step.seconds.toFixed(2)} | ${step.exitCode ?? step.signal} |`), '',
    'Gate times exclude toolchain installation and dependency download. Test times include compilation.',
    'Fixture-using test lanes share fixture-built dependencies; compare only matching lanes.',
  ].join('\n');
  fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, `${summary}\n`);
  process.exitCode = failed ? 1 : 0;
}
