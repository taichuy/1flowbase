const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { appendStepSummary, GITHUB_STEP_SUMMARY_MAX_BYTES } = require('../report-output.js');

test('large Unicode failures fit the GitHub summary byte limit without modifying the complete report', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'quality-step-summary-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const summary = path.join(root, 'summary.md');
  const report = '# Failed assertion\n' + '完整诊断🙂\n'.repeat(100_000);
  fs.writeFileSync(summary, '# Existing step output\n');
  appendStepSummary(report, summary);
  const content = fs.readFileSync(summary, 'utf8');
  assert.ok(fs.statSync(summary).size <= GITHUB_STEP_SUMMARY_MAX_BYTES);
  assert.match(content, /^# Existing step output\n\n# Failed assertion/u);
  assert.match(content, /complete report and logs remain in the workflow artifact/u);
  assert.doesNotMatch(content, /\uFFFD/u);
  assert.ok(Buffer.byteLength(report, 'utf8') > GITHUB_STEP_SUMMARY_MAX_BYTES);
  appendStepSummary(report, summary);
  assert.ok(fs.statSync(summary).size <= GITHUB_STEP_SUMMARY_MAX_BYTES);
});

test('ordinary step summaries retain their exact content', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'quality-step-summary-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const summary = path.join(root, 'summary.md');
  appendStepSummary('# Passed\nThree tests passed.', summary);
  assert.equal(fs.readFileSync(summary, 'utf8'), '\n# Passed\nThree tests passed.\n');
});
