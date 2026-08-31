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
  'api/apps/api-server/src/extension_bus/interface_contributions.rs',
  'api/apps/api-server/src/routes/identity/auth.rs',
  'api/apps/api-server/src/routes/identity/sign_in_interface.rs',
  'api/apps/api-server/src/routes/application_public_api/native.rs',
  'api/apps/api-server/src/routes/application_public_api/compatibility_interface.rs',
  'api/apps/api-server/src/routes/mcp_protocol.rs',
];

function fixture(overrides = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'interface-lifecycle-boundary-'));
  const defaults = {
    [sources[0]]: 'public_mcp_runtime_invoker_for_actor(&state);',
    [sources[1]]: 'compatibility_interface::invoke_blocking(); compatibility_interface::invoke_stream();',
    [sources[2]]: 'compatibility_interface::invoke_blocking(); compatibility_interface::invoke_stream();',
    [sources[3]]: 'boot_snapshot.authenticate(activated, credential);',
    [sources[4]]: 'struct InterfaceRegistryContribution { registry: Arc<CompiledInterfaceRegistry> }',
    [sources[5]]: 'struct PublicProvidersAdapter { store: MainDurableStore }',
    [sources[6]]: 'struct PublicSignInAdapter { store: MainDurableStore }',
    [sources[7]]: 'struct NativeAdapter { execution: Arc<dyn NativeExecutionService> }',
    [sources[8]]: 'struct CompatibilityAdapter { execution: Arc<dyn CompatibilityExecutionService> }',
    [sources[9]]: 'struct McpAdapter { dispatch: Arc<dyn McpDispatchService> }',
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

test('rejects global state contribution callbacks and Adapter service locators', () => {
  const root = fixture({
    [sources[4]]: 'type CompileInterfaceRegistry = fn(Weak<crate::app_state::ApiState>);',
    [sources[5]]: 'struct PublicProvidersAdapter { state: Weak<ApiState> }',
  });
  const violations = inspectInterfaceLifecycleBoundary(root);
  assert.ok(violations.some((value) => value.includes('global-state compile callback')));
  assert.ok(violations.some((value) => value.includes('public auth Interface adapter')));
});

test('rejects ApiState Port implementations, aliases, and trait-object casts', () => {
  const root = fixture({
    [sources[4]]: 'compile_native_interface_registry(state.clone() as Arc<dyn ApplicationNativeRunPort>);',
    [sources[7]]: 'impl ApplicationNativeRunPort for ApiState {}',
    [sources[8]]: 'type HiddenState = ApiState; impl CompatibilityBlockingPort for HiddenState {}',
  });
  const violations = inspectInterfaceLifecycleBoundary(root);
  assert.ok(violations.some((value) => value.includes('casts the global ApiState')));
  assert.ok(violations.some((value) => value.includes('native erases the global ApiState')));
  assert.ok(violations.some((value) => value.includes('compatibility erases the global ApiState')));
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
