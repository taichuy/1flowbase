const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');

const { _internal } = require('../core.js');

const fixturesRoot = path.join(__dirname, 'fixtures');

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
