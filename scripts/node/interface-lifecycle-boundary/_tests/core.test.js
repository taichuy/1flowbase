const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { inspectInterfaceLifecycleBoundary } = require('../core');

const sources = [
  'api/apps/api-server/src/routes/application_public_api/compat_sse.rs',
  'api/apps/api-server/src/routes/application_public_api/openai.rs',
  'api/apps/api-server/src/routes/application_public_api/anthropic.rs',
  'api/apps/api-server/src/routes/application_public_api/ex.rs',
];

function fixture(overrides = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'interface-lifecycle-boundary-'));
  const defaults = {
    [sources[0]]: 'public_mcp_runtime_invoker_for_actor(&state);',
    [sources[1]]: 'compatibility_interface::invoke_blocking(); compatibility_interface::invoke_stream();',
    [sources[2]]: 'compatibility_interface::invoke_blocking(); compatibility_interface::invoke_stream();',
    [sources[3]]: 'boot_snapshot.authenticate(activated, credential);',
  };
  for (const source of sources) {
    const target = path.join(root, source);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, overrides[source] ?? defaults[source]);
  }
  return root;
}

test('accepts exactly-one canonical compatibility owners', () => {
  assert.deepEqual(inspectInterfaceLifecycleBoundary(fixture()), []);
});

test('rejects legacy stream owner and direct workflow authentication', () => {
  const root = fixture({
    [sources[0]]: 'fn start_compatible_turn_stream() {}',
    [sources[3]]: 'require_session(&state); require_csrf(&headers);',
  });
  const violations = inspectInterfaceLifecycleBoundary(root);
  assert.ok(violations.some((value) => value.includes('start_compatible_turn_stream')));
  assert.ok(violations.some((value) => value.includes('exactly one frozen Authentication')));
  assert.ok(violations.some((value) => value.includes('direct authentication or CSRF bypass')));
});
