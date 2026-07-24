'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const {
  claudeInvocation,
  codexInvocation,
  TEXT_PROMPT,
  TOOL_SENTINEL,
  opencodeInvocation,
} = require('../invocations');

// Root #1377 AC-006/008: invocation flags are the locally verified no-user-config contracts.
test('Codex invocation is ephemeral, ignores user config/rules, and disables provider websockets', () => {
  const paths = { config: '/tmp/codex-config', output: '/tmp/codex-output' };
  const plan = codexInvocation('/bin/codex', paths, 'http://127.0.0.1:41002', {
    model: 'fixture-model',
  });
  assert.deepEqual(plan.args.slice(0, 7), [
    'exec', '--ephemeral', '--ignore-user-config', '--ignore-rules', '--skip-git-repo-check', '--json', '--sandbox',
  ]);
  assert.ok(plan.args.includes('model_providers.oneflowbase_gateway.supports_websockets=false'));
  assert.ok(plan.args.includes('model_providers.oneflowbase_gateway.request_max_retries=0'));
  assert.ok(plan.args.includes('model_providers.oneflowbase_gateway.stream_max_retries=0'));
  assert.ok(plan.args.includes('model_providers.oneflowbase_gateway.base_url="http://127.0.0.1:41002/v1"'));
  assert.equal(plan.args.at(-1), TEXT_PROMPT);
});

test('Claude text/tool turns are isolated and only the tool turn enables client-owned Read', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'claude-plan-'));
  const paths = { config: path.join(root, 'config'), output: path.join(root, 'output') };
  fs.mkdirSync(paths.config);
  fs.mkdirSync(paths.output);
  try {
    const plan = claudeInvocation('/bin/claude', paths, { model: 'fixture-model' }, 'text');
    const tool = claudeInvocation('/bin/claude', paths, { model: 'fixture-model' }, 'tool');
    for (const flag of ['--bare', '--no-session-persistence', '--settings', '--output-format', '--disable-slash-commands']) {
      assert.ok(plan.args.includes(flag));
    }
    assert.equal(plan.args[plan.args.indexOf('--tools') + 1], '');
    assert.equal(tool.args[tool.args.indexOf('--tools') + 1], 'Read');
    const promptIndex = tool.args.indexOf('-p');
    assert.notEqual(promptIndex, -1);
    assert.match(tool.args[promptIndex + 1], /1flowbase-client-tool-vector/u);
    assert.match(tool.args[promptIndex + 1], new RegExp(TOOL_SENTINEL, 'u'));
    assert.deepEqual(tool.args.slice(promptIndex + 2), [
      '--no-session-persistence', '--settings', tool.settingsPath,
      '--output-format', 'stream-json', '--include-partial-messages', '--verbose',
      '--model', 'fixture-model', '--tools', 'Read', '--disable-slash-commands', '--no-chrome',
    ]);
    assert.equal(plan.args[plan.args.indexOf('--output-format') + 1], 'stream-json');
    assert.ok(plan.args.includes('--include-partial-messages'));
    assert.ok(plan.args.includes('--verbose'));
    assert.deepEqual(JSON.parse(fs.readFileSync(plan.settingsPath)), {});
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('OpenCode invocation uses its real TUI so PTY output remains incremental', () => {
  const paths = { output: '/tmp/opencode-output' };
  const plan = opencodeInvocation('/bin/opencode', paths, { model: 'fixture-model' });
  assert.deepEqual(plan.args.slice(0, 5), [
    '/tmp/opencode-output', '--mini', '--model', 'oneflowbase_gateway/fixture-model', '--prompt',
  ]);
  assert.equal(plan.args.at(-1), TEXT_PROMPT);
  assert.equal(plan.terminateAfterSecondMarker, true);
});

test('Codex and OpenCode tool turns carry the deterministic local vector path', () => {
  const paths = { config: '/tmp/client-config', output: '/tmp/client-output' };
  const codex = codexInvocation('/bin/codex', paths, 'http://127.0.0.1:41002', {
    model: 'fixture-model',
  }, 'tool');
  const opencode = opencodeInvocation('/bin/opencode', paths, { model: 'fixture-model' }, 'tool');
  for (const plan of [codex, opencode]) {
    assert.match(plan.args.at(-1), /TOOL_VECTOR_PATH=\/tmp\/client-output\/tool-vector\.txt/u);
    assert.doesNotMatch(plan.args.at(-1), /Do not call any tools/u);
  }
});
