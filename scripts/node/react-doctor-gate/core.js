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
      '--no-score',
      '--fail-on',
      'warning',
      '--verbose',
      '--no-color',
    ],
    cwd: repoRoot,
  };
}

function resolveReactDoctorDiffBase({
  repoRoot = getRepoRoot(),
  env = process.env,
  spawnSyncImpl = spawnSync,
} = {}) {
  const configuredBase = env.REACT_DOCTOR_DIFF_BASE?.trim();
  if (configuredBase) {
    return configuredBase;
  }

  const result = spawnSyncImpl('git', ['rev-parse', '--verify', 'HEAD^'], {
    cwd: repoRoot,
    env,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const parentSha = result.stdout?.trim();
  if (result.error || result.status !== 0 || !parentSha) {
    const detail = result.stderr?.trim() || result.error?.message || 'unknown git error';
    throw new Error(`cannot resolve React Doctor parent baseline: ${detail}`);
  }
  return parentSha;
}

function writeReactDoctorReports({
  repoRoot,
  env = process.env,
  command,
  exitCode,
  stdout,
  stderr,
}) {
  const outputDir = resolveOutputDir(repoRoot, env);
  fs.mkdirSync(outputDir, { recursive: true });

  const logPath = path.join(outputDir, 'react-doctor.log');
  const jsonPath = path.join(outputDir, 'react-doctor.json');
  const markdownPath = path.join(outputDir, 'react-doctor.md');
  const status = exitCode === 0 ? 'passed' : 'failed';
  const log = stripAnsiControlSequences(`${stdout || ''}${stderr || ''}`);
  const report = {
    status,
    exitCode,
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
  const diffBase = resolveReactDoctorDiffBase({ repoRoot, env, spawnSyncImpl });
  const command = buildReactDoctorCommand({ repoRoot, diffBase });
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
  });

  return exitCode;
}

module.exports = {
  buildReactDoctorCommand,
  resolveReactDoctorDiffBase,
  runReactDoctorGate,
  stripAnsiControlSequences,
  writeReactDoctorReports,
};
