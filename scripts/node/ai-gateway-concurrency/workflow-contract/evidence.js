'use strict';

const fs = require('node:fs');
const path = require('node:path');

const ARTIFACT_RELATIVE_ROOT = path.join('tmp', 'test-governance', 'ai-gateway-concurrency');
const READY_FILE_NAME = 'gateway-ready.json';
const RESULT_FILE_NAME = 'workflow-result.json';

function evidencePaths(repoRoot) {
  const root = path.join(repoRoot, ARTIFACT_RELATIVE_ROOT);
  return {
    root,
    readyFile: path.join(root, READY_FILE_NAME),
    resultFile: path.join(root, RESULT_FILE_NAME),
  };
}

function prepareEvidence(repoRoot) {
  const paths = evidencePaths(repoRoot);
  fs.mkdirSync(paths.root, { recursive: true });
  return paths;
}

function writeJson(filePath, value, mode = 0o644) {
  const temporary = `${filePath}.${process.pid}.tmp`;
  fs.writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, { mode });
  fs.renameSync(temporary, filePath);
}

function publicError(error, secrets) {
  let message = String(error?.message ?? error ?? 'unknown failure').slice(0, 4_000);
  for (const secret of secrets) {
    if (typeof secret === 'string' && secret) message = message.replaceAll(secret, '<redacted>');
  }
  return { type: error?.name ?? 'Error', message };
}

function workflowResultBase(inputs) {
  return {
    schema_version: '1flowbase.ai-gateway-concurrency-workflow/v1',
    profile: inputs.profile,
    main_source_sha: inputs.mainSourceSha,
    official_source_sha: inputs.officialSourceSha,
    host_target: inputs.hostTarget,
    client_versions: { codex: '0.144.1', claude_code: '2.1.212' },
    performance_budget_applied: false,
  };
}

function appendJobSummary(summaryPath, result) {
  if (!summaryPath) return;
  const lines = [
    '# AI Gateway Concurrency',
    '',
    `- Status: **${result.status.toUpperCase()}**`,
    `- Profile: \`${result.profile}\``,
    `- Main SHA: \`${result.main_source_sha}\``,
    `- Official plugins SHA: \`${result.official_source_sha}\``,
    `- Host target: \`${result.host_target}\``,
    '- Performance timings are observations; no absolute budget was applied.',
  ];
  if (result.error) lines.push(`- Error: ${result.error.type}: ${result.error.message}`);
  fs.appendFileSync(summaryPath, `${lines.join('\n')}\n\n`, 'utf8');
}

function finalizeEvidence({ repoRoot, summaryPath, fallback }) {
  const paths = prepareEvidence(repoRoot);
  let result = fallback;
  if (fs.existsSync(paths.resultFile)) result = JSON.parse(fs.readFileSync(paths.resultFile, 'utf8'));
  else writeJson(paths.resultFile, result);
  appendJobSummary(summaryPath, result);
  fs.rmSync(paths.readyFile, { force: true });
  return result;
}

module.exports = {
  ARTIFACT_RELATIVE_ROOT,
  appendJobSummary,
  evidencePaths,
  finalizeEvidence,
  prepareEvidence,
  publicError,
  workflowResultBase,
  writeJson,
};
