const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const {
  loadVerifyRuntimeConfig,
  withHeavyVerifyLock,
} = require('./verify-runtime.js');
const { buildNodePreferredEnv } = require('./node-runtime.js');

const RUN_COMMAND_SEQUENCE_MAX_BUFFER_BYTES = 16 * 1024 * 1024;
const ANSI_CONTROL_SEQUENCE_PATTERN = /\u001b(?:\[[0-?]*[ -/]*[@-~]|\][^\u0007]*(?:\u0007|\u001b\\)|[@-Z\\-_])/gu;
const TURBO_TELEMETRY_LINES = new Set([
  'Attention:',
  'Turborepo now collects completely anonymous telemetry regarding usage.',
  'This information is used to shape the Turborepo roadmap and prioritize features.',
  "You can learn more, including how to opt-out if you'd not like to participate in this anonymous program, by visiting the following URL:",
  'https://turborepo.dev/docs/telemetry',
]);
const CARGO_PROGRESS_LINE_PATTERN = /^\s*(Updating|Downloading|Downloaded|Compiling|Checking|Finished|Fresh|Running|Doc-tests|Blocking|Waiting)\b/u;
const CARGO_LLVM_COV_INFO_LINE_PATTERN = /^info: (cargo-llvm-cov currently setting cfg\(coverage\)|running `rustup component add llvm-tools-preview\b|downloading component llvm-tools\b)/u;
const PACKAGE_MANAGER_COMMAND_ECHO_PATTERN = /^\$\s+(?:\.{1,2}\/|[A-Za-z0-9_@./-]+)(?:\s|$)/u;
const TURBO_VERSION_LINE_PATTERN = /^[•·]\s+turbo\s+\d+\.\d+\.\d+\s*$/u;
const VITEST_STDERR_HEADER_PATTERN = /^stderr\s+\|/u;
const REACT_ACT_WARNING_HEADER_PATTERN =
  /^An update to .+ inside a test was not wrapped in act\(\.\.\.\)\.?$/u;
const REACT_ACT_WARNING_END_PATTERN =
  /^This ensures that you're testing the behavior the user would see in the browser\. Learn more at https:\/\/react\.dev\/link\/wrap-tests-with-act$/u;

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function getAvailableParallelism() {
  if (typeof os.availableParallelism === 'function') {
    return os.availableParallelism();
  }

  return os.cpus().length;
}

function getCargoParallelism() {
  return Math.max(1, Math.floor(getAvailableParallelism() / 2));
}

function buildCargoCommandEnv({
  cargoParallelism,
  disableIncremental = false,
  incremental,
}) {
  const env = {
    CARGO_BUILD_JOBS: String(cargoParallelism),
  };

  if (incremental === true) {
    env.CARGO_INCREMENTAL = '1';
  } else if (incremental === false || disableIncremental) {
    env.CARGO_INCREMENTAL = '0';
  }

  return env;
}

function resolveOutputDir(repoRoot, env = process.env) {
  const override = env.ONEFLOWBASE_WARNING_OUTPUT_DIR;

  if (!override) {
    return path.join(repoRoot, 'tmp', 'test-governance');
  }

  return path.isAbsolute(override) ? override : path.resolve(repoRoot, override);
}

function ensureOutputDir(repoRoot, env = process.env) {
  const outputDir = resolveOutputDir(repoRoot, env);
  fs.mkdirSync(outputDir, { recursive: true });
  return outputDir;
}

function writeWarningCapture({
  repoRoot,
  env = process.env,
  scope,
  step,
  stderr = '',
}) {
  if (!stderr) {
    return null;
  }

  const outputDir = ensureOutputDir(repoRoot, env);
  const logPath = path.join(outputDir, `${scope}.warnings.log`);
  const sections = [`step=${step}`, '[stderr]', stderr.trimEnd()];

  fs.appendFileSync(logPath, `${sections.join('\n')}\n\n`, 'utf8');
  return logPath;
}

function stripAnsiControlSequences(value) {
  return value.replace(ANSI_CONTROL_SEQUENCE_PATTERN, '');
}

function isKnownSuccessfulToolNoiseLine(line) {
  const normalized = stripAnsiControlSequences(line).trim();

  if (!normalized) {
    return true;
  }

  return TURBO_TELEMETRY_LINES.has(normalized)
    || TURBO_VERSION_LINE_PATTERN.test(normalized)
    || PACKAGE_MANAGER_COMMAND_ECHO_PATTERN.test(normalized)
    || CARGO_PROGRESS_LINE_PATTERN.test(normalized)
    || CARGO_LLVM_COV_INFO_LINE_PATTERN.test(normalized);
}

function filterSuccessfulWarningStderr(stderr) {
  if (!stderr) {
    return '';
  }

  const retained = [];
  let skippingReactActWarning = false;
  let pendingVitestHeader = null;

  for (const line of stderr.split(/\r?\n/u)) {
    const normalized = stripAnsiControlSequences(line).trim();

    if (skippingReactActWarning) {
      if (REACT_ACT_WARNING_END_PATTERN.test(normalized)) {
        skippingReactActWarning = false;
      }
      continue;
    }

    if (isKnownSuccessfulToolNoiseLine(line)) {
      continue;
    }

    if (VITEST_STDERR_HEADER_PATTERN.test(normalized)) {
      pendingVitestHeader = line;
      continue;
    }

    if (REACT_ACT_WARNING_HEADER_PATTERN.test(normalized)) {
      skippingReactActWarning = true;
      continue;
    }

    if (pendingVitestHeader) {
      retained.push(pendingVitestHeader);
      pendingVitestHeader = null;
    }
    retained.push(line);
  }

  return retained.join('\n').trimEnd();
}

function resolveCwd(repoRoot, cwd) {
  if (!cwd) {
    return repoRoot;
  }

  return path.isAbsolute(cwd) ? cwd : path.resolve(repoRoot, cwd);
}

function clearWarningCapture(repoRoot, env, scope) {
  const outputDir = resolveOutputDir(repoRoot, env);
  const logPath = path.join(outputDir, `${scope}.warnings.log`);

  fs.rmSync(logPath, { force: true });
}

function runCommandSequence({
  repoRoot = getRepoRoot(),
  env = process.env,
  scope,
  commands,
  spawnSyncImpl = spawnSync,
  writeStdout = (text) => process.stdout.write(text),
  writeStderr = (text) => process.stderr.write(text),
  nowImpl = () => Date.now(),
  onCommandComplete,
}) {
  clearWarningCapture(repoRoot, env, scope);

  for (const command of commands) {
    const mergedEnv = {
      ...env,
      ...(command.env ?? {}),
    };
    const commandEnv = command.command === 'pnpm'
      ? buildNodePreferredEnv(mergedEnv).env
      : mergedEnv;
    const startedAtMs = nowImpl();
    const result = spawnSyncImpl(command.command, command.args, {
      cwd: resolveCwd(repoRoot, command.cwd),
      env: commandEnv,
      encoding: 'utf8',
      maxBuffer: RUN_COMMAND_SEQUENCE_MAX_BUFFER_BYTES,
      stdio: ['inherit', 'pipe', 'pipe'],
    });

    if (result.error) {
      throw result.error;
    }
    const finishedAtMs = nowImpl();

    if (result.stdout) {
      writeStdout(result.stdout);
    }

    if (result.stderr) {
      writeStderr(result.stderr);
    }

    const warningStderr = result.status === 0
      ? filterSuccessfulWarningStderr(result.stderr)
      : result.stderr;

    if (warningStderr) {
      writeWarningCapture({
        repoRoot,
        env,
        scope,
        step: command.label,
        stderr: warningStderr,
      });
    }

    if (onCommandComplete) {
      onCommandComplete({
        command,
        result,
        startedAtMs,
        finishedAtMs,
      });
    }

    if (result.status !== 0) {
      return result.status ?? 1;
    }
  }

  return 0;
}

async function runManagedCommandSequence({
  repoRoot = getRepoRoot(),
  env = process.env,
  scope,
  commandDisplay = scope,
  commands,
  lockMode = 'none',
  runtimeConfig,
  spawnSyncImpl = spawnSync,
  writeStdout = (text) => process.stdout.write(text),
  writeStderr = (text) => process.stderr.write(text),
  withHeavyVerifyLockImpl = withHeavyVerifyLock,
  runCommandSequenceImpl = runCommandSequence,
} = {}) {
  const execute = (sequenceEnv) => runCommandSequenceImpl({
    repoRoot,
    env: sequenceEnv,
    scope,
    commands,
    spawnSyncImpl,
    writeStdout,
    writeStderr,
  });

  if (lockMode !== 'heavy') {
    return execute(env);
  }

  const resolvedRuntimeConfig = runtimeConfig
    ?? loadVerifyRuntimeConfig({ repoRoot, env });

  return withHeavyVerifyLockImpl(
    {
      repoRoot,
      env,
      scope,
      command: commandDisplay,
      runtimeConfig: resolvedRuntimeConfig,
      writeStdout,
    },
    execute
  );
}

module.exports = {
  buildCargoCommandEnv,
  getRepoRoot,
  getAvailableParallelism,
  getCargoParallelism,
  resolveOutputDir,
  writeWarningCapture,
  runCommandSequence,
  runManagedCommandSequence,
};
