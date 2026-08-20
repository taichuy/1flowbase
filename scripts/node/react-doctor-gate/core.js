const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const {
  getRepoRoot,
  resolveOutputDir,
} = require('../testing/warning-capture.js');

const REACT_DOCTOR_PACKAGE = 'react-doctor@0.2.16';
const MAX_OUTPUT_BYTES = 64 * 1024 * 1024;
const ANSI_CONTROL_SEQUENCE_PATTERN = /\u001b(?:\[[0-?]*[ -/]*[@-~]|\][^\u0007]*(?:\u0007|\u001b\\)|[@-Z\\-_])/gu;

function toRepoRelative(repoRoot, filePath) {
  return path.relative(repoRoot, filePath).replace(/\\/gu, '/');
}

function stripAnsiControlSequences(value) {
  return value.replace(ANSI_CONTROL_SEQUENCE_PATTERN, '');
}

function formatCommand(command) {
  return [command.command, ...command.args].join(' ');
}

function buildReactDoctorCommand({
  repoRoot = getRepoRoot(),
  diffBase,
} = {}) {
  return {
    command: 'npm',
    args: [
      'exec',
      '--yes',
      '--package',
      REACT_DOCTOR_PACKAGE,
      '--',
      'react-doctor',
      'web/app',
      '--diff',
      diffBase,
      '--json',
      '--no-score',
      '--fail-on',
      'none',
      '--verbose',
      '--no-color',
    ],
    cwd: repoRoot,
  };
}

function resolveReactDoctorDiffBase({
  env = process.env,
} = {}) {
  const configuredBase = env.REACT_DOCTOR_DIFF_BASE?.trim();
  if (configuredBase) {
    return configuredBase;
  }

  const pullRequestBase = env.GITHUB_BASE_SHA?.trim();
  if (pullRequestBase) {
    return pullRequestBase;
  }
  throw new Error(
    'React Doctor range is incomplete: set REACT_DOCTOR_DIFF_BASE or GITHUB_BASE_SHA',
  );
}

function resolveReactDoctorCandidate({ env = process.env } = {}) {
  const candidate = env.REACT_DOCTOR_CANDIDATE_SHA?.trim() || env.GITHUB_SHA?.trim();
  if (candidate) {
    return candidate;
  }
  throw new Error('React Doctor range is incomplete: set REACT_DOCTOR_CANDIDATE_SHA or GITHUB_SHA');
}

function resolveGitRevision({ repoRoot, revision, label, env, spawnSyncImpl }) {
  const result = spawnSyncImpl('git', ['rev-parse', '--verify', `${revision}^{commit}`], {
    cwd: repoRoot,
    env,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const resolved = result.stdout?.trim();
  if (result.error || result.status !== 0 || !resolved) {
    const detail = result.stderr?.trim() || result.error?.message || 'unknown git error';
    throw new Error(`cannot resolve React Doctor ${label} \`${revision}\`: ${detail}`);
  }
  return resolved;
}

function resolveReactDoctorRange({
  repoRoot = getRepoRoot(),
  env = process.env,
  spawnSyncImpl = spawnSync,
} = {}) {
  const base = resolveReactDoctorDiffBase({ env });
  const candidate = resolveReactDoctorCandidate({ env });
  const baseSha = resolveGitRevision({
    repoRoot,
    revision: base,
    label: 'base',
    env,
    spawnSyncImpl,
  });
  const candidateSha = resolveGitRevision({
    repoRoot,
    revision: candidate,
    label: 'candidate',
    env,
    spawnSyncImpl,
  });
  const headSha = resolveGitRevision({
    repoRoot,
    revision: 'HEAD',
    label: 'checked-out candidate',
    env,
    spawnSyncImpl,
  });
  if (candidateSha !== headSha) {
    throw new Error(
      `React Doctor candidate ${candidateSha} is not the checked-out candidate ${headSha}`,
    );
  }
  const diff = spawnSyncImpl(
    'git',
    ['diff', '-z', '--name-only', '--diff-filter=ACMR', baseSha, candidateSha, '--', 'web/app'],
    {
      cwd: repoRoot,
      env,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    },
  );
  if (diff.error || diff.status !== 0) {
    const detail = diff.stderr?.trim() || diff.error?.message || 'unknown git error';
    throw new Error(`cannot resolve React Doctor changed files: ${detail}`);
  }
  const changedFiles = (diff.stdout || '')
    .split('\0')
    .map((filePath) => filePath.trim())
    .filter(Boolean);
  return { baseSha, candidateSha, changedFiles };
}

function countConfiguredSuppressions({ repoRoot, changedFiles }) {
  const configPath = path.join(repoRoot, 'web', 'app', 'doctor.config.json');
  try {
    const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
    const changedAppFiles = new Set(
      changedFiles
        .filter((filePath) => filePath.startsWith('web/app/'))
        .map((filePath) => filePath.slice('web/app/'.length)),
    );
    return (config.ignore?.overrides || []).reduce((count, override) => {
      if (!override.files?.some((filePath) => changedAppFiles.has(filePath))) {
        return count;
      }
      return count + (Array.isArray(override.rules) ? override.rules.length : 0);
    }, 0);
  } catch {
    return null;
  }
}

function parseDoctorJsonReport(stdout) {
  const start = stdout.indexOf('{');
  if (start < 0) {
    return null;
  }
  try {
    return JSON.parse(stdout.slice(start));
  } catch {
    return null;
  }
}

function resolveSuppressedDiagnostics(doctorReport) {
  return doctorReport?.summary?.suppressedDiagnosticCount ?? null;
}

function resolveBaseSource(env = process.env) {
  return env.REACT_DOCTOR_BASE_SOURCE?.trim() || 'explicit-base-input';
}

function resolveCandidateSource(env = process.env) {
  return env.REACT_DOCTOR_CANDIDATE_SOURCE?.trim() || 'checked-out-candidate';
}

function writeReactDoctorReports({
  repoRoot,
  env = process.env,
  command,
  exitCode,
  stdout,
  stderr,
  range,
}) {
  const outputDir = resolveOutputDir(repoRoot, env);
  fs.mkdirSync(outputDir, { recursive: true });

  const logPath = path.join(outputDir, 'react-doctor.log');
  const jsonPath = path.join(outputDir, 'react-doctor.json');
  const markdownPath = path.join(outputDir, 'react-doctor.md');
  const status = exitCode === 0 ? 'passed' : 'failed';
  const log = stripAnsiControlSequences(`${stdout || ''}${stderr || ''}`);
  const doctorReport = parseDoctorJsonReport(stdout || '');
  const configuredSuppressionEntries = countConfiguredSuppressions({
    repoRoot,
    changedFiles: range.changedFiles,
  });
  const report = {
    status,
    exitCode,
    baseSha: range.baseSha,
    candidateSha: range.candidateSha,
    changedFiles: range.changedFiles,
    changedFileCount: range.changedFiles.length,
    unsuppressedDiagnostics: doctorReport?.summary?.totalDiagnosticCount ?? null,
    suppressedDiagnostics: resolveSuppressedDiagnostics(doctorReport),
    configuredSuppressionEntries,
    suppressionSource: 'react-doctor JSON report when available; otherwise unavailable',
    configuredSuppressionSource:
      'web/app/doctor.config.json override entries intersecting changed files',
    baseSource: resolveBaseSource(env),
    candidateSource: resolveCandidateSource(env),
    command: formatCommand(command),
    cwd: toRepoRelative(repoRoot, command.cwd),
    logPath: toRepoRelative(repoRoot, logPath),
    markdownPath: toRepoRelative(repoRoot, markdownPath),
    reportPath: toRepoRelative(repoRoot, jsonPath),
    stdoutBytes: Buffer.byteLength(stdout || '', 'utf8'),
    stderrBytes: Buffer.byteLength(stderr || '', 'utf8'),
  };

  const markdown = [
    '# React Doctor Gate',
    '',
    `- Status: ${status}`,
    `- Exit code: ${exitCode}`,
    `- Base SHA: ${report.baseSha}`,
    `- Candidate SHA: ${report.candidateSha}`,
    `- Base source: ${report.baseSource}`,
    `- Candidate source: ${report.candidateSource}`,
    `- Changed files: ${report.changedFileCount}`,
    `- Unsuppressed diagnostics: ${report.unsuppressedDiagnostics ?? 'unavailable'}`,
    `- Suppressed diagnostics: ${report.suppressedDiagnostics ?? 'unavailable'}`,
    `- Configured suppression entries: ${report.configuredSuppressionEntries ?? 'unavailable'}`,
    `- Command: \`${report.command}\``,
    `- Log: ${report.logPath}`,
    `- JSON: ${report.reportPath}`,
    '',
  ].join('\n');

  fs.writeFileSync(logPath, log, 'utf8');
  fs.writeFileSync(jsonPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  fs.writeFileSync(markdownPath, markdown, 'utf8');

  return report;
}

function runReactDoctorGate({
  repoRoot = getRepoRoot(),
  env = process.env,
  spawnSyncImpl = spawnSync,
  writeStdout = (text) => process.stdout.write(text),
  writeStderr = (text) => process.stderr.write(text),
} = {}) {
  const range = resolveReactDoctorRange({ repoRoot, env, spawnSyncImpl });
  const command = buildReactDoctorCommand({ repoRoot, diffBase: range.baseSha });
  const result = spawnSyncImpl(command.command, command.args, {
    cwd: command.cwd,
    env,
    encoding: 'utf8',
    maxBuffer: MAX_OUTPUT_BYTES,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const stdout = result.stdout || '';
  const stderr = result.error
    ? `${result.stderr || ''}${result.error.stack || result.error.message}\n`
    : result.stderr || '';
  const exitCode = result.error ? 1 : result.status ?? 1;

  if (stdout) {
    writeStdout(stdout);
  }

  if (stderr) {
    writeStderr(stderr);
  }

  writeReactDoctorReports({
    repoRoot,
    env,
    command,
    exitCode,
    stdout,
    stderr,
    range,
  });

  return exitCode;
}

module.exports = {
  buildReactDoctorCommand,
  countConfiguredSuppressions,
  parseDoctorJsonReport,
  resolveBaseSource,
  resolveCandidateSource,
  resolveSuppressedDiagnostics,
  resolveReactDoctorDiffBase,
  resolveReactDoctorRange,
  resolveReactDoctorCandidate,
  runReactDoctorGate,
  stripAnsiControlSequences,
  writeReactDoctorReports,
};
