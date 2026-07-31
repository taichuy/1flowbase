const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');

const { _internal } = require('../core.js');

const fixturesRoot = path.join(__dirname, 'fixtures');
const sixProviderMatrix = require('../fixtures/six-provider-matrix.json');

function providerFor(manifest) {
  return {
    provider_code: manifest.plugin_id,
    plugin_id: manifest.plugin_id,
    expected_manifest: {
      contract_version: manifest.contract_version,
      consumption_kind: manifest.consumption_kind,
      execution_mode: manifest.execution_mode,
      slot_codes: manifest.slot_codes,
      runtime: manifest.runtime,
    },
  };
}

test(
  'Root #1366 AC-002/004/005/006/007 parses runtime blocks and canonicalizes omitted capabilities',
  () => {
    const fullRuntime = _internal.parseManifestFacts(
      path.join(fixturesRoot, 'full-runtime.yaml')
    );
    const omittedCapabilities = _internal.parseManifestFacts(
      path.join(fixturesRoot, 'runtime-without-capabilities.yaml')
    );

    assert.deepEqual(fullRuntime.runtime, {
      protocol: 'stdio_json',
      entry: 'bin/full-runtime-provider',
      capabilities: ['end_user_reference', 'system_prompt_blocks'],
    });
    assert.deepEqual(omittedCapabilities.runtime, {
      protocol: 'stdio_json',
      entry: 'bin/runtime-without-capabilities-provider',
      capabilities: [],
    });

    assert.doesNotThrow(() =>
      _internal.assertManifest(providerFor(omittedCapabilities), omittedCapabilities)
    );

    const wrongEntry = {
      ...fullRuntime,
      runtime: {
        ...fullRuntime.runtime,
        entry: 'bin/wrong-provider',
      },
    };
    assert.throws(
      () => _internal.assertManifest(providerFor(fullRuntime), wrongEntry),
      /actual package manifest does not match fixture_full_runtime fixture/u
    );
  }
);

test('runner diagnostics are bounded and redact every conformance canary', () => {
  const diagnostic = _internal.boundedRunnerError(
    { body: { message: `failed secret-value ${'x'.repeat(700)}` } },
    { secret: 'secret-value' }
  );

  assert.equal(diagnostic.includes('secret-value'), false);
  assert.match(diagnostic, /\[REDACTED\]/u);
  assert.equal(diagnostic.length, 512);
});

test('Anthropic fake SSE completes with the vendor message_stop event', () => {
  const body = _internal.fakeResponseBody('anthropic_messages_sse');

  assert.match(body, /"type":"message_stop"/u);
  assert.doesNotMatch(body, /data: \[DONE\]/u);
});

test('six-provider wire only restores protocol context for declared profiles', () => {
  const providers = new Map(
    sixProviderMatrix.providers.map((provider) => [provider.provider_code, provider])
  );

  for (const providerCode of ['openai', 'anthropic', 'openai_compatible']) {
    assert.equal(
      providers.get(providerCode).expected_wire.headers['x-claude-code-session-id'],
      '$HEADER_CANARY'
    );
  }
  for (const providerCode of ['aliyun_bailian', 'deepseek', 'gemini']) {
    assert.equal(
      Object.hasOwn(
        providers.get(providerCode).expected_wire.headers,
        'x-claude-code-session-id'
      ),
      false
    );
  }
});
