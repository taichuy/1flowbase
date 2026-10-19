const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { buildReactDoctorCommand, resolveReactDoctorScanMode, runReactDoctorGate } = require('../core.js');

test('project health forces a complete scan even when a baseline is supplied', () => {
  const args = buildReactDoctorCommand({ repoRoot: '/repo', diffBase: 'old-base' }).args;
  assert.ok(args.includes('--full'));
  assert.ok(!args.includes('--diff'));
  assert.equal(resolveReactDoctorScanMode({ env: {} }), 'full');
  assert.equal(resolveReactDoctorScanMode({ env: { GITHUB_EVENT_NAME: 'pull_request' } }), 'diff');
  assert.equal(resolveReactDoctorScanMode({ env: { GITHUB_EVENT_NAME: 'pull_request', REACT_DOCTOR_SCAN_MODE: 'full' } }), 'full');
  assert.throws(() => resolveReactDoctorScanMode({ env: { REACT_DOCTOR_SCAN_MODE: 'subset' } }), /full or diff/u);
});

for (const reportedMode of ['full', 'diff', undefined]) {
  test(`full acceptance rejects a narrowed or missing tool mode: ${reportedMode}`, () => {
    const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'doctor-full-'));
    try {
      fs.mkdirSync(path.join(repoRoot, 'web/app'), { recursive: true });
      fs.writeFileSync(path.join(repoRoot, 'web/app/doctor.config.json'), JSON.stringify({ ignore: { overrides: [] } }));
      const status = runReactDoctorGate({
        repoRoot,
        env: { REACT_DOCTOR_DIFF_BASE: 'base', REACT_DOCTOR_CANDIDATE_SHA: 'candidate' },
        spawnSyncImpl(command, args) {
          if (command === 'git') return { status: 0, stdout: args[0] === 'diff' ? '' : args.at(-1) === 'base^{commit}' ? 'base' : 'candidate' };
          assert.ok(args.includes('--full'));
          return { status: 0, stdout: JSON.stringify({ mode: reportedMode, summary: { totalDiagnosticCount: 0 }, diagnostics: [] }) };
        },
        writeStdout() {}, writeStderr() {},
      });
      assert.equal(status, reportedMode === 'full' ? 0 : 1);
      const receipt = JSON.parse(fs.readFileSync(path.join(repoRoot, 'tmp/test-governance/react-doctor.json'), 'utf8'));
      assert.equal(receipt.scanMode, 'full');
      assert.equal(receipt.reportedScanMode, reportedMode ?? null);
      if (reportedMode !== 'full') assert.equal(receipt.diagnosticReview.status, 'invalid-scan-mode');
    } finally {
      fs.rmSync(repoRoot, { recursive: true, force: true });
    }
  });
}
