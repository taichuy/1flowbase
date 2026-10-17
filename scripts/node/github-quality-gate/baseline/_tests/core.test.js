const assert = require('node:assert/strict');
const fs = require('node:fs');
const test = require('node:test');
const path = require('node:path');
const { DEFAULT_AGGREGATE_SCOPES } = require('../../commands.js');
const { acceptsReceipt, createGitHubClient, resolveBaseline, selectScheduledBaseline, writeBaseline } = require('../core.js');
const { main } = require('../cli.js');

const candidate = 'a'.repeat(40);
const previous = 'b'.repeat(40);
const run = { id: 12, status: 'completed', conclusion: 'success', head_branch: 'main', head_sha: 'c'.repeat(40), html_url: 'https://github.com/owner/repo/actions/runs/12' };
function receipt(overrides = {}) {
  return { scope: 'ci', status: 'passed', exitCode: 0, branch: 'latest', commit: previous,
    runUrl: run.html_url, components: DEFAULT_AGGREGATE_SCOPES.map((scope) => ({ scope, status: 'passed', exitCode: 0, commit: previous })), ...overrides };
}
function accepts(value, target = run) {
  return acceptsReceipt({ receipt: value, run: target, branch: 'latest', candidate });
}

test('complete receipt verifies latest independently of main workflow source SHA', () => {
  assert.equal(accepts(receipt()), true);
  const result = selectScheduledBaseline({ client: { listRuns: () => [run], readReceipt: () => receipt() }, branch: 'latest', candidate });
  assert.deepEqual(result, { baseSha: previous, baseSource: 'previous-successful-quality-gate-run:12' });
  assert.notEqual(result.baseSha, run.head_sha);
});

test('local scope and superficially passing incomplete CI are not protected baselines', () => {
  for (const value of [receipt({ scope: 'repo-tooling' }), receipt({ components: receipt().components.slice(1) }), receipt({ components: [] })]) {
    assert.equal(accepts(value), false);
  }
});

test('baseline rejects mismatched candidate, branch, run provenance and aggregate status', () => {
  for (const override of [
    { commit: candidate }, { commit: 'short' }, { branch: 'dev' }, { runUrl: `${run.html_url}0` },
    { status: 'failed' }, { exitCode: 1 }, { exitCode: '0' },
  ]) assert.equal(accepts(receipt(override)), false, JSON.stringify(override));
});

test('every component must carry passing evidence for the aggregate candidate without duplicate scopes', () => {
  for (const override of [{ commit: candidate }, { commit: '' }, { status: 'failed' }, { exitCode: 1 }]) {
    const value = receipt();
    value.components[0] = { ...value.components[0], ...override };
    assert.equal(accepts(value), false);
  }
  const duplicate = receipt();
  duplicate.components.push(duplicate.components[0]);
  assert.equal(accepts(duplicate), false);
});

test('bounded search skips current and partial runs, and follows API pages', () => {
  const queried = [];
  const read = [];
  const newer = { ...run, id: 13, html_url: `${run.html_url}3` };
  const result = selectScheduledBaseline({ candidate, branch: 'latest', currentRunId: 13, perPage: 1, maxPages: 2,
    client: { listRuns(page) { queried.push(page); return [page === 1 ? newer : run]; },
      readReceipt(value) { read.push(value.id); return receipt(); } } });
  assert.equal(result.baseSha, previous);
  assert.deepEqual(queried, [1, 2]);
  assert.deepEqual(read, [12]);
});

test('missing or invalid artifact cannot fallback to workflow SHA or HEAD parent', () => {
  const warnings = [];
  assert.throws(() => selectScheduledBaseline({ candidate, branch: 'latest', maxPages: 1, perPage: 1,
    client: { listRuns: () => [run], readReceipt: () => { throw new Error('expired download'); } }, onRejected: (value) => warnings.push(value) }), /No complete passing.*within 1 pages/u);
  assert.match(warnings[0], /expired download/u);
});

test('GitHub requests do not branch-filter workflow runs and read only root final receipt', () => {
  const calls = [];
  let directory;
  const client = createGitHubClient({ repository: 'owner/repo', env: {}, exec(command, args) {
    calls.push([command, args]);
    if (args[0] === 'api' && args[1].includes('/workflows/')) return JSON.stringify({ workflow_runs: [run] });
    if (args[0] === 'api') return JSON.stringify({ total_count: 1, artifacts: [{ name: 'test-governance-artifacts', expired: false }] });
    directory = args[args.indexOf('--dir') + 1];
    fs.mkdirSync(path.join(directory, 'parallel', 'old'), { recursive: true });
    fs.writeFileSync(path.join(directory, 'parallel', 'old', 'quality-gate-report.json'), JSON.stringify(receipt({ commit: candidate })));
    fs.writeFileSync(path.join(directory, 'quality-gate-report.json'), JSON.stringify(receipt()));
    return '';
  } });
  assert.deepEqual(client.listRuns(1, 50), [run]);
  assert.equal(client.readReceipt(run).commit, previous);
  assert.doesNotMatch(calls[0][1][1], /branch=/u);
  assert.match(calls[0][1][1], /status=success&per_page=50&page=1/u);
  assert.equal(fs.existsSync(directory), false, 'temporary downloads are reclaimed');
});

test('nested-only receipts cannot satisfy the final artifact, and cleanup also follows malformed JSON', () => {
  for (const malformed of [false, true]) {
    let directory;
    const client = createGitHubClient({ repository: 'owner/repo', exec(_command, args) {
      if (args[0] === 'api') return JSON.stringify({ total_count: 1, artifacts: [{ name: 'test-governance-artifacts' }] });
      directory = args[args.indexOf('--dir') + 1];
      fs.mkdirSync(path.join(directory, 'old'));
      fs.writeFileSync(path.join(directory, 'old', 'quality-gate-report.json'), JSON.stringify(receipt()));
      if (malformed) fs.writeFileSync(path.join(directory, 'quality-gate-report.json'), '{invalid');
      return '';
    } });
    if (malformed) assert.throws(() => client.readReceipt(run), SyntaxError);
    else assert.equal(client.readReceipt(run), null);
    assert.equal(fs.existsSync(directory), false);
  }
});

test('duplicate, expired and missing final artifacts never download', () => {
  for (const response of [
    { total_count: 0, artifacts: [] },
    { total_count: 2, artifacts: [{ name: 'test-governance-artifacts' }, { name: 'test-governance-artifacts' }] },
    { total_count: 1, artifacts: [{ name: 'test-governance-artifacts', expired: true }] },
  ]) {
    const client = createGitHubClient({ repository: 'owner/repo', exec(_command, args) {
      assert.equal(args[0], 'api');
      return JSON.stringify(response);
    } });
    assert.equal(client.readReceipt(run), null);
  }
});

test('manual execution requires an explicit full baseline and does not contact GitHub', () => {
  const env = { GITHUB_EVENT_NAME: 'workflow_dispatch', QUALITY_GATE_TARGET_SHA: candidate };
  assert.throws(() => resolveBaseline({ env }), /require inputs.react_doctor_base/u);
  assert.throws(() => resolveBaseline({ env: { ...env, REACT_DOCTOR_MANUAL_BASE: 'HEAD^' } }), /full commit SHA/u);
  const calls = [];
  const result = resolveBaseline({ env: { ...env, REACT_DOCTOR_MANUAL_BASE: previous }, exec(command, args) {
    calls.push([command, args]); return `${previous}\n`;
  } });
  assert.deepEqual(calls, [['git', ['rev-parse', '--verify', `${previous}^{commit}`]]]);
  assert.deepEqual(result, { baseSha: previous, baseSource: 'workflow_dispatch.react_doctor_base' });
});

test('scheduled resolver consumes receipt identity and publishes only resolved output', () => {
  const env = { GITHUB_EVENT_NAME: 'schedule', QUALITY_GATE_TARGET_SHA: candidate, QUALITY_GATE_TARGET_BRANCH: 'latest', GITHUB_RUN_ID: '99', GITHUB_ENV: '/example/env' };
  const result = resolveBaseline({ env, client: { listRuns: () => [run], readReceipt: () => receipt() } });
  const writes = [];
  writeBaseline(result, { env, append: (...args) => writes.push(args) });
  assert.deepEqual(writes, [['/example/env', `REACT_DOCTOR_DIFF_BASE=${previous}\nREACT_DOCTOR_BASE_SOURCE=previous-successful-quality-gate-run:12\n`, 'utf8']]);
  assert.throws(() => writeBaseline(result, { env: {} }), /GITHUB_ENV/u);
});

test('CLI fails visibly without publishing a guessed baseline', () => {
  let wrote = false;
  let error = '';
  const status = main({ env: {}, resolve: () => { throw new Error('No complete passing receipt'); },
    write: () => { wrote = true; }, stderr: { write(value) { error += value; } } });
  assert.equal(status, 1);
  assert.equal(wrote, false);
  assert.match(error, /No complete passing receipt/u);
});
