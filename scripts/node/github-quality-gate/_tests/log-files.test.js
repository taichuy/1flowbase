const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { once } = require('node:events');
const { runQualityGate } = require('../core.js');
const { runGateCommand, combineGateLogs, readFailureExcerpt } = require('../log-files.js');

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gate-log-files-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  return root;
}

test('real gate output beyond the old 64 MiB limit retains complete logs and failure report', async (t) => {
  const repoRoot = fixture(t);
  const scripts = path.join(repoRoot, 'scripts/node');
  fs.mkdirSync(scripts, { recursive: true });
  fs.writeFileSync(path.join(scripts, 'verify-backend.js'), `
    const fs = require('node:fs');
    const chunk = Buffer.alloc(65536, 120);
    for (let i = 1023; i < chunk.length; i += 1024) chunk[i] = 10;
    fs.writeSync(1, 'HEADER\\n');
    for (let i = 0; i < 1056; i++) fs.writeSync(1, chunk);
    fs.writeSync(1, 'TAIL\\n');
    fs.writeSync(2, 'failure: 完整诊断\\n');
    process.exitCode = 23;
  `);
  let stdoutBytes = 0;
  let stderr = '';
  const result = await runQualityGate({
    repoRoot, scope: 'backend', env: { GITHUB_SHA: 'a'.repeat(40) },
    writeStdout(text) { stdoutBytes += Buffer.byteLength(text); },
    writeStderr(text) { stderr += text; },
  });
  const logPath = path.join(repoRoot, 'tmp/test-governance/quality-gate.latest.log');
  const expectedStdoutBytes = 1056 * 65536 + Buffer.byteLength('HEADER\nTAIL\n');
  assert.equal(result.exitCode, 23);
  assert.equal(result.status, 'failed');
  assert.equal(stdoutBytes, expectedStdoutBytes);
  assert.equal(stderr, 'failure: 完整诊断\n');
  assert.equal(fs.statSync(logPath).size, expectedStdoutBytes + Buffer.byteLength(stderr));
  const descriptor = fs.openSync(logPath, 'r');
  const header = Buffer.alloc(7);
  fs.readSync(descriptor, header, 0, 7, 0);
  fs.closeSync(descriptor);
  assert.equal(header.toString(), 'HEADER\n');
  assert.match(fs.readFileSync(result.reportPath, 'utf8'), /完整诊断/u);
  assert.match(readFailureExcerpt(logPath), /TAIL/u);
  assert.deepEqual(fs.readdirSync(path.dirname(logPath)).filter((name) => name.startsWith('.gate-stderr-')), []);
});

test('descriptor replay preserves UTF-8 across chunks, stdout/stderr separation and success', async (t) => {
  const root = fixture(t);
  let stdout = '';
  let stderr = '';
  const status = await runGateCommand({
    commandSpec: { command: process.execPath, args: ['-e', `
      const fs = require('node:fs');
      fs.writeSync(1, 'x'.repeat(65535) + '边界😀\\n');
      fs.writeSync(2, 'y'.repeat(65535) + '错误😀\\n');
    `], cwd: root },
    logPath: path.join(root, 'gate.log'),
    writeStdout(text) { stdout += text; },
    writeStderr(text) { stderr += text; },
  });
  assert.equal(status, 0);
  assert.equal(stdout, `${'x'.repeat(65535)}边界😀\n`);
  assert.equal(stderr, `${'y'.repeat(65535)}错误😀\n`);
  assert.equal(fs.readFileSync(path.join(root, 'gate.log'), 'utf8'), stdout + stderr);
});

test('spawn failure and signals retain diagnostics and fail instead of producing success', async (t) => {
  const root = fixture(t);
  const logPath = path.join(root, 'gate.log');
  const common = { logPath, writeStdout() {}, writeStderr() {} };
  assert.equal(await runGateCommand({ ...common,
    commandSpec: { command: path.join(root, 'does-not-exist'), args: [], cwd: root },
  }), 1);
  assert.match(fs.readFileSync(logPath, 'utf8'), /ENOENT/u);
  if (process.platform !== 'win32') {
    assert.equal(await runGateCommand({ ...common,
      commandSpec: { command: process.execPath, args: ['-e', "require('node:fs').writeSync(2, 'before signal\\n'); process.kill(process.pid, 'SIGTERM')"], cwd: root },
    }), 1);
    assert.match(fs.readFileSync(logPath, 'utf8'), /before signal\nCommand terminated by SIGTERM/u);
  }
});

test('aggregate logs retain component order, complete bytes and missing-log headers', async (t) => {
  const root = fixture(t);
  const artifacts = ['one', 'two', 'missing'].map((scope) => ({ scope, artifactName: `test-${scope}`, artifactPath: path.join(root, scope) }));
  for (const artifact of artifacts.slice(0, 2)) {
    fs.mkdirSync(artifact.artifactPath);
    fs.writeFileSync(path.join(artifact.artifactPath, 'quality-gate.latest.log'), `${artifact.scope}\n\n`);
  }
  const logPath = path.join(root, 'combined.log');
  await combineGateLogs(logPath, artifacts);
  assert.equal(fs.readFileSync(logPath, 'utf8'), '===== one (test-one) =====\none\n\n\n\n===== two (test-two) =====\ntwo\n\n\n\n===== missing (test-missing) =====\n');
});

test('native console replay waits for a slow pipe without EAGAIN or truncated output', { timeout: 10000 }, async (t) => {
  const root = fixture(t);
  const modulePath = require.resolve('../log-files.js');
  const code = `
    const { runGateCommand } = require(${JSON.stringify(modulePath)});
    runGateCommand({
      commandSpec: { command: process.execPath, args: ['-e', 'require("node:fs").writeSync(1, "x".repeat(2 * 1024 * 1024)); require("node:fs").writeSync(2, "complete\\\\n");'] },
      logPath: ${JSON.stringify(path.join(root, 'gate.log'))}
    }).then((status) => { process.exitCode = status; });
  `;
  const child = spawn(process.execPath, ['-e', code], { stdio: ['ignore', 'pipe', 'pipe'] });
  t.after(() => { if (child.exitCode === null) child.kill(); });
  let stdoutBytes = 0;
  let stderr = '';
  child.stdout.on('data', (chunk) => { stdoutBytes += chunk.length; });
  child.stderr.on('data', (chunk) => { stderr += chunk; });
  child.stdout.pause();
  const resume = setTimeout(() => child.stdout.resume(), 300);
  t.after(() => clearTimeout(resume));
  const [exitCode, signal] = await once(child, 'close');
  assert.equal(signal, null);
  assert.equal(exitCode, 0, stderr);
  assert.equal(stdoutBytes, 2 * 1024 * 1024);
  assert.equal(stderr, 'complete\n');
});

test('console delivery failure preserves both artifact streams and removes temporary files', async (t) => {
  const root = fixture(t);
  const logPath = path.join(root, 'gate.log');
  await assert.rejects(runGateCommand({
    commandSpec: { command: process.execPath, args: ['-e', 'console.log("out"); console.error("diagnostic")'] },
    logPath,
    writeStdout() { throw new Error('closed console'); },
  }), /closed console/u);
  assert.equal(fs.readFileSync(logPath, 'utf8'), 'out\ndiagnostic\n');
  assert.deepEqual(fs.readdirSync(root), ['gate.log']);
});

test('failure excerpts preserve anchor priority, distant summary and tail across line boundaries', (t) => {
  const root = fixture(t);
  const logPath = path.join(root, 'gate.log');
  const write = (lines) => fs.writeFileSync(logPath, lines.join('\n'));
  write(['before', 'context', 'thread panicked at example.rs', 'failures:', ...Array.from({ length: 100 }, (_, i) => `detail-${i}`), '\x1b[31mtest result: FAILED. 1 passed; 1 failed;\x1b[0m', ...Array(100).fill('compiling'), ...Array(100).fill('')]);
  const excerpt = readFailureExcerpt(logPath);
  assert.match(excerpt, /^failures:\ndetail-0/u);
  assert.match(excerpt, /\.\.\.\ntest result: FAILED\. 1 passed; 1 failed;$/u);
  assert.doesNotMatch(excerpt, /compiling|\x1b/u);
  write(['before', 'context', 'thread panicked at example.rs', 'last']);
  assert.equal(readFailureExcerpt(logPath), 'before\ncontext\nthread panicked at example.rs\nlast');
  write(['x'.repeat(65535), '中文\r', 'last', ...Array(100).fill('')]);
  assert.equal(readFailureExcerpt(logPath), `${'x'.repeat(65535)}\n中文\nlast`);
  assert.equal(readFailureExcerpt(path.join(root, 'missing')), '');
});
