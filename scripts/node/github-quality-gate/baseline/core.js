const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { DEFAULT_AGGREGATE_SCOPES } = require('../commands.js');

const COMMIT = /^[a-f0-9]{40}$/u;
const ARTIFACT_NAME = 'test-governance-artifacts';

function acceptsReceipt({ receipt, run, branch, candidate, expectedScopes = DEFAULT_AGGREGATE_SCOPES }) {
  if (!receipt || receipt.scope !== 'ci' || receipt.status !== 'passed' || receipt.exitCode !== 0
    || receipt.branch !== branch || !COMMIT.test(receipt.commit || '') || receipt.commit === candidate
    || receipt.runUrl !== run.html_url || !Array.isArray(receipt.components) || !receipt.components.length) return false;
  const scopes = new Set();
  for (const component of receipt.components) {
    if (!component || !component.scope || scopes.has(component.scope) || component.status !== 'passed'
      || component.exitCode !== 0 || component.commit !== receipt.commit) return false;
    scopes.add(component.scope);
  }
  return expectedScopes.every((scope) => scopes.has(scope));
}

function createGitHubClient({ repository, env = process.env, exec = execFileSync } = {}) {
  if (!/^[\w.-]+\/[\w.-]+$/u.test(repository || '')) throw new Error('GITHUB_REPOSITORY is required');
  const command = (args) => exec('gh', args, { env, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 });
  const api = (endpoint) => JSON.parse(command(['api', endpoint]));
  return {
    listRuns(page, perPage) {
      // The workflow may execute on main while its receipt verifies latest.
      // GitHub's branch/head_sha describe workflow source, not the tested candidate.
      return api(`repos/${repository}/actions/workflows/quality-gate.yml/runs?status=success&per_page=${perPage}&page=${page}`).workflow_runs;
    },
    readReceipt(run) {
      // Two results suffice to reject ambiguous duplicate final artifacts.
      const response = api(`repos/${repository}/actions/runs/${run.id}/artifacts?name=${ARTIFACT_NAME}&per_page=2`);
      const artifacts = response.artifacts || [];
      if (response.total_count !== 1 || artifacts.length !== 1 || artifacts[0].name !== ARTIFACT_NAME || artifacts[0].expired) return null;
      const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'quality-baseline-'));
      try {
        command(['run', 'download', String(run.id), '--repo', repository, '--name', ARTIFACT_NAME, '--dir', directory]);
        // Never recursively discover receipts: nested parallel/ contains older components.
        const receiptPath = path.join(directory, 'quality-gate-report.json');
        return fs.existsSync(receiptPath) ? JSON.parse(fs.readFileSync(receiptPath, 'utf8')) : null;
      } finally {
        fs.rmSync(directory, { recursive: true, force: true });
      }
    },
  };
}

function selectScheduledBaseline({ client, branch, candidate, currentRunId, maxPages = 2, perPage = 50,
  expectedScopes = DEFAULT_AGGREGATE_SCOPES, onRejected = () => {} }) {
  if (!COMMIT.test(candidate || '')) throw new Error('A full candidate SHA is required');
  if (!branch || /[\r\n]/u.test(branch)) throw new Error('Target branch is required');
  if (!Number.isInteger(maxPages) || maxPages < 1 || !Number.isInteger(perPage) || perPage < 1 || perPage > 100) {
    throw new Error('Invalid GitHub baseline search pagination');
  }
  for (let page = 1; page <= maxPages; page += 1) {
    const runs = client.listRuns(page, perPage);
    if (!Array.isArray(runs)) throw new Error('Invalid GitHub workflow run response');
    for (const run of runs) {
      if (String(run.id) === String(currentRunId) || run.status !== 'completed' || run.conclusion !== 'success') continue;
      try {
        const receipt = client.readReceipt(run);
        if (acceptsReceipt({ receipt, run, branch, candidate, expectedScopes })) {
          return { baseSha: receipt.commit, baseSource: `previous-successful-quality-gate-run:${run.id}` };
        }
      } catch (error) {
        onRejected(`Run ${run.id}: ${error.message}`);
      }
    }
    if (runs.length < perPage) break;
  }
  throw new Error(`No complete passing quality-gate receipt baseline for ${branch} within ${maxPages} pages`);
}

function resolveBaseline({ env = process.env, client, exec = execFileSync, onRejected } = {}) {
  const candidate = env.QUALITY_GATE_TARGET_SHA || env.REACT_DOCTOR_CANDIDATE_SHA || env.GITHUB_SHA;
  if (!COMMIT.test(candidate || '')) throw new Error('A full candidate SHA is required');
  if (env.GITHUB_EVENT_NAME === 'workflow_dispatch') {
    const base = (env.REACT_DOCTOR_MANUAL_BASE || env.REACT_DOCTOR_DIFF_BASE || '').trim();
    if (!base) throw new Error('React Doctor manual runs require inputs.react_doctor_base (protected baseline SHA)');
    if (!COMMIT.test(base)) throw new Error('React Doctor manual baseline must be a full commit SHA');
    const resolved = exec('git', ['rev-parse', '--verify', `${base}^{commit}`], { env, encoding: 'utf8', maxBuffer: 1024 * 1024 }).trim();
    if (resolved !== base) throw new Error('React Doctor manual baseline does not resolve to its supplied commit');
    return { baseSha: base, baseSource: 'workflow_dispatch.react_doctor_base' };
  }
  if (env.GITHUB_EVENT_NAME !== 'schedule') throw new Error('Baseline resolution supports schedule or workflow_dispatch');
  return selectScheduledBaseline({ client: client || createGitHubClient({ repository: env.GITHUB_REPOSITORY, env, exec }),
    branch: env.QUALITY_GATE_TARGET_BRANCH, candidate, currentRunId: env.GITHUB_RUN_ID, onRejected });
}

function writeBaseline(result, { env = process.env, append = fs.appendFileSync } = {}) {
  if (!env.GITHUB_ENV) throw new Error('GITHUB_ENV is required to publish the React Doctor baseline');
  if (!COMMIT.test(result.baseSha) || /[\r\n]/u.test(result.baseSource)) throw new Error('Invalid baseline output');
  append(env.GITHUB_ENV, `REACT_DOCTOR_DIFF_BASE=${result.baseSha}\nREACT_DOCTOR_BASE_SOURCE=${result.baseSource}\n`, 'utf8');
}

module.exports = { acceptsReceipt, createGitHubClient, resolveBaseline, selectScheduledBaseline, writeBaseline };
