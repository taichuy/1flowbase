'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { claudeInvocation, codexInvocation, FIXED_PROMPT } = require('../invocations');

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
  assert.ok(plan.args.includes('model_providers.oneflowbase_gateway.base_url="http://127.0.0.1:41002/v1"'));
  assert.equal(plan.args.at(-1), FIXED_PROMPT);
});

test('Claude invocation is bare, stateless, explicit-settings, stream-json, and tool-free', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'claude-plan-'));
  const paths = { config: path.join(root, 'config'), output: path.join(root, 'output') };
  fs.mkdirSync(paths.config);
  fs.mkdirSync(paths.output);
  try {
    const plan = claudeInvocation('/bin/claude', paths, { model: 'fixture-model' });
    for (const flag of ['--bare', '--no-session-persistence', '--settings', '--output-format', '--disable-slash-commands']) {
      assert.ok(plan.args.includes(flag));
    }
    assert.equal(plan.args[plan.args.indexOf('--tools') + 1], '');
    assert.equal(plan.args[plan.args.indexOf('--output-format') + 1], 'stream-json');
    assert.deepEqual(JSON.parse(fs.readFileSync(plan.settingsPath)), {});
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
