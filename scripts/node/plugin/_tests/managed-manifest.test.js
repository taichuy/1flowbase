const test = require('node:test');
const assert = require('node:assert/strict');
const { createManifestTemplate } = require('../manifest.js');

test('root_2007_ac_001_002_manifest_routes: scaffold preserves explicit contribution bindings and legacy generation', () => {
  const managed = {
    module: {
      bus_version: 'v1', module_id: 'fixture', module_version: '0.1.0', module_kind: 'runtime',
      contributions: ['compute', 'render'].map(id => ({ contribution_id: `fixture.${id}`, contributor_module_id: 'fixture', point_id: `acme.${id}`, contract_version: 'acme/v1', mode: 'append' })),
    },
    execution_bindings: ['compute', 'render'].map(handler => ({ contribution_id: `fixture.${handler}`, execution_mode: 'process_per_call', runtime: { protocol: 'stdio_json', entry: 'bin/fixture-provider' }, handler })),
  };
  const raw = createManifestTemplate({ pluginCode: 'fixture', pluginName: 'Fixture', managed });
  assert.match(raw, /^manifest_version: 2$/m);
  assert.match(raw, /^slot_codes: \[\]$/m);
  assert.deepEqual(JSON.parse(raw.match(/^managed: (.*)$/m)[1]), managed);
  const old = createManifestTemplate({ pluginCode: 'fixture', pluginName: 'Fixture' });
  assert.match(old, /^manifest_version: 1$/m);
  assert.match(old, /slot_codes:\n  - model_provider/);
  assert.throws(() => createManifestTemplate({ pluginCode: 'other', pluginName: 'Other', managed }), /identity/);
});
