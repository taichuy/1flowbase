const path = require('node:path');

const {
  buildCargoCommandEnv,
  getRepoRoot,
  runManagedCommandSequence,
} = require('../testing/warning-capture.js');
const { loadVerifyRuntimeConfig } = require('../testing/verify-runtime.js');
const { resolveNodeBinaryFromPath } = require('../testing/node-runtime.js');

const DEFAULT_STATE_PROTOCOL_MODEL = '1flowbase';
const DEFAULT_STATE_PROTOCOL_OUT_DIR = path.join('tmp', 'test-governance', 'state-protocols', 'acp-claude');

function resolveScriptsNodeEntry(repoRoot, entryName) {
  return path.join(repoRoot, 'scripts', 'node', entryName);
}

function resolveScriptsNodeCliEntry(repoRoot, entryName) {
  return `${resolveScriptsNodeEntry(repoRoot, entryName)}.js`;
}

function resolveScriptsNodePackedCliEntry(repoRoot, entryName) {
  return path.join(repoRoot, 'scripts', 'node', 'cli', `${entryName}.js`);
}

function takeStateProtocolValue(argv, index, flag) {
  const value = argv[index + 1];
  if (!value || value.startsWith('--')) {
    throw new Error(`${flag} requires a value`);
  }
  return value;
}

function parseStateProtocolsCliArgs(argv = []) {
  const options = {
    help: false,
    model: DEFAULT_STATE_PROTOCOL_MODEL,
    outDir: DEFAULT_STATE_PROTOCOL_OUT_DIR,
    skipLiveAcp: false,
    ensureBackend: true,
    timeoutMs: 180000,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '-h' || arg === '--help') {
      options.help = true;
      continue;
    }

    if (arg === '--model') {
      options.model = takeStateProtocolValue(argv, index, arg);
      index += 1;
      continue;
    }

    if (arg === '--out-dir') {
      options.outDir = takeStateProtocolValue(argv, index, arg);
      index += 1;
      continue;
    }

    if (arg === '--timeout-ms') {
      options.timeoutMs = Number(takeStateProtocolValue(argv, index, arg));
      index += 1;
      continue;
    }

    if (arg === '--skip-live-acp') {
      options.skipLiveAcp = true;
      continue;
    }

    if (arg === '--no-ensure-backend') {
      options.ensureBackend = false;
      continue;
    }

    throw new Error(`Unknown state protocol option: ${arg}`);
  }

  if (!Number.isFinite(options.timeoutMs) || options.timeoutMs <= 0) {
    throw new Error('--timeout-ms must be a positive number');
  }

  return options;
}

function buildStateProtocolCommands({
  repoRoot,
  env = process.env,
  cargoJobs,
  cargoTestThreads,
  incremental = false,
  options = parseStateProtocolsCliArgs([]),
}) {
  const nodeBinary = resolveNodeBinaryFromPath(env);
  const commands = [
    {
      label: 'state-protocol-acp-smoke-unit-tests',
      command: nodeBinary,
      args: [
        '--test',
        path.join(repoRoot, 'scripts', 'node', 'acp-claude-smoke', '_tests', 'core.test.js'),
      ],
      cwd: repoRoot,
    },
    {
      label: 'state-protocol-anthropic-api-tests',
      command: 'cargo',
      args: [
        'test',
        '-p',
        'api-server',
        '--jobs',
        String(cargoJobs),
        'anthropic_',
        '--',
        `--test-threads=${cargoTestThreads}`,
      ],
      cwd: 'api',
      env: buildCargoCommandEnv({ cargoParallelism: cargoJobs, incremental }),
    },
  ];

  if (options.skipLiveAcp) {
    return commands;
  }

  if (options.ensureBackend) {
    commands.push({
      label: 'state-protocol-backend-ensure',
      command: nodeBinary,
      args: [
        resolveScriptsNodeCliEntry(repoRoot, 'dev-up'),
        'ensure',
        '--backend-only',
        '--skip-docker',
      ],
      cwd: repoRoot,
    });
  }

  commands.push({
    label: 'state-protocol-acp-claude-smoke',
    command: nodeBinary,
    args: [
      resolveScriptsNodePackedCliEntry(repoRoot, 'acp-claude-smoke'),
      '--model',
      options.model,
      '--out-dir',
      options.outDir,
      '--timeout-ms',
      String(options.timeoutMs),
    ],
    cwd: repoRoot,
  });

  return commands;
}

function usageStateProtocols(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/verify-state-protocols.js [--model <id>] [--out-dir <dir>] [--timeout-ms <ms>] [--skip-live-acp] [--no-ensure-backend]\n'
      + 'Runs fixed state protocol regressions: Anthropic projection tests plus real Claude Code ACP thought/message smoke.\n'
  );
}

async function runStateProtocols(argv = [], deps = {}) {
  const options = parseStateProtocolsCliArgs(argv);

  if (options.help) {
    usageStateProtocols(deps.writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const runtimeConfig = deps.runtimeConfig || loadVerifyRuntimeConfig({ repoRoot, env });
  const managedRunner = deps.managedRunnerImpl || runManagedCommandSequence;
  const commandSuffix = [
    options.model === DEFAULT_STATE_PROTOCOL_MODEL ? '' : `--model ${options.model}`,
    options.outDir === DEFAULT_STATE_PROTOCOL_OUT_DIR ? '' : `--out-dir ${options.outDir}`,
    options.timeoutMs === 180000 ? '' : `--timeout-ms ${options.timeoutMs}`,
    options.skipLiveAcp ? '--skip-live-acp' : '',
    options.ensureBackend ? '' : '--no-ensure-backend',
  ].filter(Boolean).join(' ');

  return managedRunner({
    repoRoot,
    env,
    scope: 'verify-state-protocols',
    lockMode: 'heavy',
    commandDisplay: `node scripts/node/verify-state-protocols.js${commandSuffix ? ` ${commandSuffix}` : ''}`,
    runtimeConfig,
    commands: buildStateProtocolCommands({
      repoRoot,
      env,
      cargoJobs: runtimeConfig.backend.cargoJobs,
      cargoTestThreads: runtimeConfig.backend.cargoTestThreads,
      incremental: runtimeConfig.backend.incremental,
      options,
    }),
    spawnSyncImpl: deps.spawnSyncImpl,
    writeStdout: deps.writeStdout,
    writeStderr: deps.writeStderr,
  });
}

module.exports = {
  buildStateProtocolCommands,
  parseStateProtocolsCliArgs,
  runStateProtocols,
};
