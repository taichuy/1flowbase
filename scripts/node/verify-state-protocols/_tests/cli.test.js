const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');

const {
  buildCommands,
  parseStateProtocolsCliArgs,
} = require('../../verify-state-protocols.js');

test('parseStateProtocolsCliArgs defaults to hard live ACP regression', () => {
  assert.deepEqual(parseStateProtocolsCliArgs([]), {
    help: false,
    model: '1flowbase',
    outDir: path.join('tmp', 'test-governance', 'state-protocols', 'acp-claude'),
    skipLiveAcp: false,
    ensureBackend: true,
    timeoutMs: 180000,
  });
});

test('parseStateProtocolsCliArgs supports static-only protocol checks', () => {
  assert.deepEqual(
    parseStateProtocolsCliArgs([
      '--model',
      'custom-model',
      '--out-dir',
      'tmp/state-protocols',
      '--timeout-ms',
      '5000',
      '--skip-live-acp',
      '--no-ensure-backend',
    ]),
    {
      help: false,
      model: 'custom-model',
      outDir: 'tmp/state-protocols',
      skipLiveAcp: true,
      ensureBackend: false,
      timeoutMs: 5000,
    }
  );
});

test('buildCommands composes static projection tests plus backend ensure and ACP smoke', () => {
  const repoRoot = '/repo-root';
  const commands = buildCommands({
    repoRoot,
    env: {},
    cargoJobs: 4,
    cargoTestThreads: 2,
    options: parseStateProtocolsCliArgs(['--model', '1flowbase', '--timeout-ms', '5000']),
  });

  assert.deepEqual(commands.map((command) => command.label), [
    'state-protocol-acp-smoke-unit-tests',
    'state-protocol-anthropic-api-tests',
    'state-protocol-backend-ensure',
    'state-protocol-acp-claude-smoke',
  ]);
  assert.deepEqual(commands[0].args, [
    '--test',
    path.join(repoRoot, 'scripts', 'node', 'acp-claude-smoke', '_tests', 'core.test.js'),
  ]);
  assert.deepEqual(commands[1].args, [
    'test',
    '--package',
    'api-server',
    '--jobs',
    '4',
    'anthropic_',
    '--',
    '--test-threads=2',
  ]);
  assert.deepEqual(commands[2].args, [
    path.join(repoRoot, 'scripts', 'node', 'dev-up.js'),
    'ensure',
    '--backend-only',
    '--skip-docker',
  ]);
  assert.deepEqual(commands[3].args, [
    path.join(repoRoot, 'scripts', 'node', 'cli', 'acp-claude-smoke.js'),
    '--model',
    '1flowbase',
    '--out-dir',
    path.join('tmp', 'test-governance', 'state-protocols', 'acp-claude'),
    '--timeout-ms',
    '5000',
  ]);
});

test('Anthropic source migration retains both Root and Gateway test consumers', () => {
  const repoRoot = path.resolve(__dirname, '../../../..');
  const commands = buildCommands({ repoRoot, env: {}, cargoJobs: 2, cargoTestThreads: 2,
    options: parseStateProtocolsCliArgs(['--skip-live-acp']) });
  const command = commands.find((entry) => entry.label === 'state-protocol-anthropic-api-tests');
  assert.deepEqual(command.args.filter((_arg, index) => command.args[index - 1] === '--package'), ['api-server', 'api-public-gateway']);
  assert.ok(command.args.includes('anthropic_'));
});
