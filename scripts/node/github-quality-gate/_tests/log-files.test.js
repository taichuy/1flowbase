const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
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
  assert.match(fs.readFileSync(result.reportJsonPath, 'utf8'), /完整诊断/u);
  assert.match(readFailureExcerpt(logPath), /TAIL/u);
  assert.deepEqual(fs.readdirSync(path.dirname(logPath)).filter((name) => name.startsWith('.gate-stderr-')), []);
});

test('descriptor replay preserves UTF-8 across chunks, stdout/stderr separation and success', (t) => {
  const root = fixture(t);
  let stdout = '';
  let stderr = '';
  const status = runGateCommand({
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

test('spawn failure and signals retain diagnostics and fail instead of producing success', (t) => {
  const root = fixture(t);
  const logPath = path.join(root, 'gate.log');
  const common = { logPath, writeStdout() {}, writeStderr() {} };
  assert.equal(runGateCommand({ ...common,
    commandSpec: { command: path.join(root, 'does-not-exist'), args: [], cwd: root },
  }), 1);
  assert.match(fs.readFileSync(logPath, 'utf8'), /ENOENT/u);
  if (process.platform !== 'win32') {
    assert.equal(runGateCommand({ ...common,
      commandSpec: { command: process.execPath, args: ['-e', "require('node:fs').writeSync(2, 'before signal\\n'); process.kill(process.pid, 'SIGTERM')"], cwd: root },
    }), 1);
    assert.match(fs.readFileSync(logPath, 'utf8'), /before signal\nCommand terminated by SIGTERM/u);
  }
});

test('aggregate logs retain component order, complete bytes and missing-log headers', (t) => {
  const root = fixture(t);
  const artifacts = ['one', 'two', 'missing'].map((scope) => ({ scope, artifactName: `test-${scope}`, artifactPath: path.join(root, scope) }));
  for (const artifact of artifacts.slice(0, 2)) {
    fs.mkdirSync(artifact.artifactPath);
    fs.writeFileSync(path.join(artifact.artifactPath, 'quality-gate.latest.log'), `${artifact.scope}\n\n`);
  }
  const logPath = path.join(root, 'combined.log');
  combineGateLogs(logPath, artifacts);
  assert.equal(fs.readFileSync(logPath, 'utf8'), '===== one (test-one) =====\none\n\n\n\n===== two (test-two) =====\ntwo\n\n\n\n===== missing (test-missing) =====\n');
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
