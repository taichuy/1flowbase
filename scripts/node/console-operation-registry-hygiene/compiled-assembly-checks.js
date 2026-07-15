const path = require('node:path');
const { spawnSync } = require('node:child_process');

const COMPILED_ASSEMBLY_CHECKS = [
  {
    label: 'migrated-assembly-owners',
    filter: 'migrated_assembly_contains_every_console_router_owner_assembly',
  },
  {
    label: 'console-route-assembly',
    filter: 'console_route_assembly',
  },
];
const RUN_COMMAND_MAX_BUFFER_BYTES = 16 * 1024 * 1024;

function parseCargoTestCounts(output) {
  let passedCount = 0;
  let failedCount = 0;
  let ignoredCount = 0;
  const pattern = /test result:\s+(?:ok|FAILED)\.\s+(\d+) passed;\s+(\d+) failed;(?:\s+(\d+) ignored;)?/gu;
  let match = pattern.exec(output);
  while (match) {
    passedCount += Number.parseInt(match[1], 10);
    failedCount += Number.parseInt(match[2], 10);
    ignoredCount += Number.parseInt(match[3] || '0', 10);
    match = pattern.exec(output);
  }
  return {
    passedCount,
    failedCount,
    ignoredCount,
    testsRun: passedCount + failedCount,
  };
}

function buildCompiledAssemblyCommands({ repoRoot } = {}) {
  return COMPILED_ASSEMBLY_CHECKS.map((target) => ({
    label: target.label,
    command: 'cargo',
    args: [
      'test',
      '-p',
      'api-server',
      target.filter,
      '--',
      '--test-threads=1',
    ],
    cwd: path.join(repoRoot, 'api'),
  }));
}

function runCompiledAssemblyChecks({
  repoRoot,
  env = process.env,
  spawnSyncImpl = spawnSync,
  writeStdout = () => {},
  writeStderr = () => {},
  nowImpl = () => Date.now(),
} = {}) {
  const commands = buildCompiledAssemblyCommands({ repoRoot });
  const results = [];
  let status = 0;

  for (const command of commands) {
    const startedAtMs = nowImpl();
    let result;
    try {
      result = spawnSyncImpl(command.command, command.args, {
        cwd: command.cwd,
        env: {
          ...env,
          CARGO_INCREMENTAL: '0',
        },
        encoding: 'utf8',
        maxBuffer: RUN_COMMAND_MAX_BUFFER_BYTES,
        stdio: ['ignore', 'pipe', 'pipe'],
      });
    } catch (error) {
      result = {
        status: 1,
        stdout: '',
        stderr: error.message,
      };
    }
    const finishedAtMs = nowImpl();
    const stdout = result?.stdout || '';
    const stderr = result?.stderr || '';
    const counts = parseCargoTestCounts(`${stdout}\n${stderr}`);
    const exitCode = result?.status ?? 1;
    const passed = exitCode === 0 && counts.failedCount === 0 && counts.testsRun > 0;
    if (!passed) {
      status = exitCode === 0 ? 1 : exitCode;
    }
    if (stdout) {
      writeStdout(stdout);
    }
    if (stderr && !passed) {
      writeStderr(stderr);
    }
    results.push({
      label: command.label,
      command: command.command,
      args: command.args,
      cwd: path.relative(repoRoot, command.cwd).split(path.sep).join('/'),
      status: passed ? 'passed' : 'failed',
      exitCode,
      passedCount: counts.passedCount,
      failedCount: counts.failedCount,
      testsRun: counts.testsRun,
      durationMs: Math.max(0, finishedAtMs - startedAtMs),
      failureReason: passed ? null : exitCode !== 0 ? 'cargo-test-failed' : 'no-matching-test-result',
    });
  }

  return {
    status,
    authoritative: true,
    commands: results,
  };
}

module.exports = {
  buildCompiledAssemblyCommands,
  parseCargoTestCounts,
  runCompiledAssemblyChecks,
};
