const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { buildGateCommand, DEFAULT_AGGREGATE_SCOPES } = require('../commands.js');
const root = path.resolve(__dirname, '../../../..');
test('composition selector routes only to the finite runner and stays outside broad aggregates', () => {
  assert.deepEqual(buildGateCommand({ repoRoot: '/repo', scope: 'plugin-composition-2007' }), {
    command: process.execPath, args: ['/repo/scripts/node/plugin-composition-test-batch/runner.js'], cwd: '/repo',
  });
  assert.equal(DEFAULT_AGGREGATE_SCOPES.includes('plugin-composition-2007'), false);
});
test('composition workflow has one dedicated serial job and always uploads evidence', () => {
  const workflow = fs.readFileSync(path.join(root, '.github/workflows/quality-gate.yml'), 'utf8');
  assert.match(workflow, /inputs.scope != 'plugin-composition-2007'/u);
  const job = workflow.slice(workflow.indexOf('  plugin-composition-2007:\n'), workflow.indexOf('  repo-tooling-gate:\n'));
  assert.match(job, /PLUGIN_COMPOSITION_CANDIDATE_SHA: \$\{\{ inputs.candidate_sha \}\}/u);
  assert.match(job, /postgres:18-alpine/u);
  assert.match(job, /if: always\(\)/u);
  assert.doesNotMatch(job, /pnpm|llvm-cov|matrix:|quality-gate\/action|publish_issue/u);
  assert.match(job, /node scripts\/node\/plugin-composition-test-batch\/runner.js/u);
});
