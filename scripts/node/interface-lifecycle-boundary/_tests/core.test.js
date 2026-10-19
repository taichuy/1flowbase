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
  'api/apps/api-server/src/lib.rs',
  'api/apps/api-server/src/external_endpoint_catalog.rs',
  'api/apps/api-server/src/routes/application_public_api/openai/response_dispatch.rs',
  'api/apps/api-server/src/extension_bus/interface_contributions/provider_operations.rs',
  'api/apps/api-server/src/extension_bus/interface_contributions/network.rs',
  'api/apps/api-server/src/extension_bus/interface_contributions/plugins.rs',
  'api/apps/api-server/src/extension_bus/interface_contributions/mcp.rs',
  'api/apps/api-server/src/extension_bus/interface_contributions/system.rs',
];

function fixture(overrides = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'interface-lifecycle-boundary-'));
  const defaults = {
    [sources[0]]: 'public_mcp_runtime_invoker_for_actor(&state);',
    [sources[1]]: 'compatibility_interface::invoke_blocking(); compatibility_interface::invoke_client_stream_with_principal();',
    [sources[2]]: 'compatibility_interface::invoke_blocking(); compatibility_interface::invoke_client_stream_with_principal();',
    [sources[3]]: 'boot_snapshot.authenticate_invocation(snapshot, binding, protocol, credential); authenticated.into_envelope(input);',
    [sources[4]]: 'struct InterfaceRegistryContribution { registry: Arc<CompiledInterfaceRegistry> }',
    [sources[5]]: 'struct PublicProvidersAdapter { store: MainDurableStore }',
    [sources[6]]: 'struct PublicSignInAdapter { store: MainDurableStore }',
    [sources[7]]: 'struct NativeAdapter { execution: Arc<dyn NativeExecutionService> }',
    [sources[8]]: `struct CompatibilityAdapter { execution: Arc<dyn CompatibilityExecutionService> }
      fn invoke_client_stream_with_principal() {
        invoke_typed_stream_with_principal(); project_stream_invocation();
      }
      fn invoke_typed_stream_with_principal() {
        interface_runtime::InterfaceInvocationKernel::new()
          .invoke_server_stream_with_dispatch_target::<Input>(principal.into_envelope());
      }`,
    [sources[9]]: 'struct McpAdapter { dispatch: Arc<dyn McpDispatchService> }',
    [sources[10]]: 'publish_external_endpoint_catalog(); contribute_openapi_document(); fn console_router_with_assembly() { ExternalRouteAssembly::new(); router.contributions(); router.into_router(); }',
    [sources[11]]: 'fn compile_complete() {} enum Error { UnclassifiedRows }',
    [sources[12]]: '',
    [sources[13]]: '',
    [sources[14]]: '',
    [sources[15]]: '',
    [sources[16]]: '',
    [sources[17]]: '',
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

test('registry compilation may extract services but its adapter cannot retain global state', () => {
  const root = fixture({
    [sources[4]]: 'compile_registry(Arc::clone(state));',
    [sources[5]]: 'fn compile_registry(state: Arc<ApiState>) { PublicProvidersAdapter { store: state.store.clone() } } struct PublicProvidersAdapter { store: MainDurableStore }',
  });
  try {
    assert.deepEqual(inspectInterfaceLifecycleBoundary(root), []);
    const route = path.join(root, 'api/apps/api-server/src/routes/retained_adapter.rs');
    fs.writeFileSync(route, 'struct LogsAdapter { state: Arc<ApiState> }');
    assert.ok(inspectInterfaceLifecycleBoundary(root).some((item) => item.includes('retained_adapter.rs Interface adapter retains')));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('rejects a direct authentication owner outside the exact protocol/control allowlist', () => {
  const root = fixture({
    [sources[5]]: 'require_session(&state); struct PublicProvidersAdapter { store: MainDurableStore }',
  });
  const violations = inspectInterfaceLifecycleBoundary(root);
  assert.ok(violations.some((value) => value.includes('retains a direct route authentication owner')));
});

test('rejects a Composition Root without complete catalog publication', () => {
  const root = fixture({
    [sources[10]]: 'fn build_router() {}',
  });
  const violations = inspectInterfaceLifecycleBoundary(root);
  assert.ok(violations.some((value) => value.includes('complete external endpoint catalog')));
});

// Root AC-001: mutate the real production mount, not a synthetic catalog row.
test('root_1998 rejects a bare production mount after inventory extraction', () => {
  const { inspectExternalRouteAssemblyBoundary } = require('../core');
  const production = fs.readFileSync(path.resolve(__dirname, '../../../../api/apps/api-server/src/lib.rs'), 'utf8');
  assert.deepEqual(inspectExternalRouteAssemblyBoundary(production), []);
  const mutated = production.replace(/router\s*\.into_router\(\)/u, 'router.into_router().merge(axum::Router::new().route("/rogue", axum::routing::get(rogue)))');
  assert.notEqual(mutated, production);
  assert.ok(inspectExternalRouteAssemblyBoundary(mutated).some((value) => value.includes('bare external mount')));
});

test('root_1998 rejects a bare mount at the final application root', () => {
  const { inspectExternalRouteAssemblyBoundary } = require('../core');
  const production = fs.readFileSync(path.resolve(__dirname, '../../../../api/apps/api-server/src/lib.rs'), 'utf8');
  const mutated = production.replace('.layer(cors_layer(config))', '.merge(rogue_router()).layer(cors_layer(config))');
  assert.notEqual(mutated, production);
  assert.ok(inspectExternalRouteAssemblyBoundary(mutated).some((value) => value.includes('bare external mount')));
});

// A4: mutate the real route so a bypass cannot hide behind the wrapper's presence elsewhere.
test('root_1998 rejects direct authentication and discarded lineage in production source', () => {
  const { inspectAuthenticationInvocationBoundary } = require('../core');
  const file = 'api/apps/api-server/src/routes/application_public_api/native.rs';
  const production = fs.readFileSync(path.resolve(__dirname, '../../../..', file), 'utf8');
  assert.deepEqual(inspectAuthenticationInvocationBoundary(file, production), []);
  const bypass = production.replace('.authenticate_invocation', '.authenticate');
  assert.notEqual(bypass, production);
  assert.ok(inspectAuthenticationInvocationBoundary(file, bypass).some(value => value.includes('bypasses')));
  const discarded = production.replaceAll('.into_envelope(', '.discard_attempt(');
  assert.notEqual(discarded, production);
  assert.ok(inspectAuthenticationInvocationBoundary(file, discarded).some(value => value.includes('lineage')));
});


test('accepts the current production principal stream ownership chain', () => {
  assert.deepEqual(inspectInterfaceLifecycleBoundary(path.resolve(__dirname, '../../../..')), []);
});

test('rejects client streams which bypass the typed kernel or discard the principal', () => {
  const productionRoot = path.resolve(__dirname, '../../../..');
  const production = fs.readFileSync(path.join(productionRoot, sources[8]), 'utf8');
  for (const [before, after] of [
    ['invoke_typed_stream_with_principal(Arc::clone(&state)', 'bypass_stream(Arc::clone(&state)'],
    ['invoke_typed_stream_with_principal_inner(state, binding_id, principal, input).await', 'bypass_stream(state, binding_id, principal, input).await'],
    ['principal.into_envelope(binding_id, authentication_activation, input)', 'unverified_envelope(input)'],
    ['.invoke_server_stream_with_dispatch_target::<', '.bypass_dispatch::<'],
  ]) {
    const mutated = production.replace(before, after);
    assert.notEqual(mutated, production);
    const root = fixture({ [sources[8]]: mutated });
    try {
      assert.ok(inspectInterfaceLifecycleBoundary(root).some((value) => value.includes('preserve principal')));
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  }
});


test('notification refresh authentication requires its exact Protocol Control catalog declaration', () => {
  const productionRoot = path.resolve(__dirname, '../../../..');
  const route = 'api/apps/api-server/src/routes/notifications/events.rs';
  const catalog = fs.readFileSync(path.join(productionRoot, sources[11]), 'utf8');
  const control = fs.readFileSync(path.join(productionRoot, route), 'utf8');
  for (const classified of [true, false]) {
    const declaration = classified ? catalog : catalog.replace(
      'Self::NotificationEvents => ExternalEndpointContribution::protocol_control_http(',
      'Self::NotificationEvents => ExternalEndpointContribution::business_http('
    );
    if (!classified) assert.notEqual(declaration, catalog);
    const root = fixture({ [sources[11]]: declaration });
    try {
      fs.mkdirSync(path.dirname(path.join(root, route)), { recursive: true });
      fs.writeFileSync(path.join(root, route), control);
      const violations = inspectInterfaceLifecycleBoundary(root);
      if (classified) assert.deepEqual(violations, []);
      else assert.ok(violations.some((item) => item.includes(`${route} retains a direct route authentication owner`)));
      const businessRoute = 'api/apps/api-server/src/routes/notifications/business.rs';
      fs.writeFileSync(path.join(root, businessRoute), 'require_session(&state);');
      assert.ok(inspectInterfaceLifecycleBoundary(root).some((item) =>
        item.includes(`${businessRoute} retains a direct route authentication owner`)));
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  }
});
