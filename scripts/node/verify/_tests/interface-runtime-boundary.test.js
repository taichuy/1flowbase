'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const repoRoot = path.resolve(__dirname, '../../../..');
const runtimeRoot = path.join(repoRoot, 'api/crates/interface-runtime');

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');
}

const { productionRustSources } = require('./support/rust-sources.js');

function forbiddenSourceImports(source) {
  const forbidden = [
    'api_server::',
    'axum::',
    'control_plane::',
    'plugin_framework::',
    'runtime_extension_host::',
    'storage_durable::',
    'storage_durable_postgres::',
    'storage_ephemeral::',
  ];
  return forbidden.filter((pattern) => source.includes(pattern));
}

test('Delivery 1944 keeps interface-runtime dependency closed and AuthN adapter-owned', () => {
  const cargo = read('api/crates/interface-runtime/Cargo.toml');
  const production = productionRustSources(path.join(runtimeRoot, 'src'));
  const dependencies = cargo
    .match(/\[dependencies\]([\s\S]*?)(?:\n\[|$)/u)[1]
    .split('\n')
    .map((line) => line.match(/^([a-zA-Z0-9_-]+)(?:\.workspace)?\s*=/u)?.[1])
    .filter(Boolean)
    .sort();

  // Only domain is an allowed internal Cargo edge; external utility dependencies
  // and private module layout are implementation choices, not architecture APIs.
  const internalNames = fs.readdirSync(path.join(repoRoot, 'api/crates'), { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .flatMap((entry) => {
      const manifest = path.join(repoRoot, 'api/crates', entry.name, 'Cargo.toml');
      return fs.existsSync(manifest) ? [fs.readFileSync(manifest, 'utf8').match(/^name\s*=\s*"([^"]+)"/mu)?.[1]] : [];
    }).filter(Boolean);
  assert.deepEqual(dependencies.filter((name) => internalNames.includes(name)), ['domain']);
  for (const infrastructure of ['axum', 'sqlx', 'reqwest', 'storage-durable', 'storage-durable-postgres', 'storage-ephemeral']) {
    assert.equal(dependencies.includes(infrastructure), false, infrastructure);
  }
  assert.deepEqual(forbiddenSourceImports(production), []);
  assert.deepEqual(forbiddenSourceImports('use axum::http::HeaderMap;'), ['axum::']);
  assert.doesNotMatch(production, /(?:Cookie|HeaderMap|Session|ApiKey)Credential/u);
});

test('Delivery 1944 exposes the approved typed interface facade without infrastructure capabilities', () => {
  const facade = read('api/crates/interface-runtime/src/lib.rs');
  const publicModules = [...facade.matchAll(/^pub\s+mod\s+([a-z_]+);$/gmu)].map(
    (match) => match[1],
  );
  const groupedPublicSymbols = [...facade.matchAll(/pub use [a-z_]+::\{([\s\S]*?)\};/gu)]
    .flatMap((match) => match[1].split(',').map((symbol) => symbol.trim()).filter(Boolean));
  const singlePublicSymbols = [
    ...facade.matchAll(/^pub use [a-z_]+::([A-Z][A-Za-z0-9_]+);$/gmu),
  ].map((match) => match[1]);
  const publicSymbols = [...groupedPublicSymbols, ...singlePublicSymbols].sort();
  const requiredSymbols = [
    'ActivatedAuthenticationAdapter',
    'ApplicationPrincipal',
    'AuthenticationActivationIdentity',
    'CanonicalInvocationResult',
    'CompiledInvocationPlan',
    'CompiledInterfaceRegistry',
    'ContractIdentity',
    'ExecutionAttempt',
    'ExecutionTargetPin',
    'GraphFingerprint',
    'InterfaceDefinition',
    'InterfaceAdmissionContribution',
    'InterfaceAuthorizationContribution',
    'InterfaceExtensionPoint',
    'InterfaceExtensionRegistration',
    'InterfaceHandler',
    'InterfaceHandlerContext',
    'InterfaceInvocationKernel',
    'InterfaceInvocationReceipt',
    'InterfaceServerStream',
    'InterfaceStreamAccumulator',
    'ManagedInterfaceProjection',
    'InvocationEnvelope',
    'PrincipalSummary',
    'ProtocolBinding',
    'PublicPrincipal',
    'TypedInterfaceAdmissionPlan',
    'TypedInterfaceAuthorizationPlan',
    'TypedInterfaceDefinitionContribution',
    'UserPrincipal',
  ].sort();

  assert.deepEqual(publicModules, []);
  for (const symbol of requiredSymbols) assert.ok(publicSymbols.includes(symbol), symbol);
  assert.doesNotMatch(facade, /serde_json|axum|sqlx|RegistryHandle|HttpHandler/u);
  assert.match(facade, /pub use invocation::\{/u);
  assert.match(facade, /pub use hook::\{/u);
  assert.match(facade, /pub use registry::\{/u);
});

test('Delivery 1944 routes cannot inject decision or hook plans and plugin decisions only see bounded facts', () => {
  const kernel = productionRustSources(path.join(runtimeRoot, 'src/invocation.rs'));
  const decision = read('api/crates/interface-runtime/src/decision.rs');
  const routes = [
    'api/apps/api-server/src/routes/identity/auth.rs',
    'api/apps/api-server/src/routes/application_public_api/native.rs',
    'api/apps/api-server/src/routes/mcp_protocol.rs',
    'api/apps/api-server/src/routes/settings/host_infrastructure/interface_operation.rs',
  ].map(read).join('\n');

  assert.doesNotMatch(
    kernel,
    /pub async fn invoke_with_(?:hook|authorization|admission)_plan/u,
  );
  assert.doesNotMatch(
    routes,
    /invoke_with_(?:hook|authorization|admission)_plan/u,
  );
  assert.doesNotMatch(
    decision,
    /ActorContext|Cookie|HeaderMap|bearer_token|ApiState|MainDurableStore|HostInfrastructureRegistry|RuntimeHost/u,
  );
  assert.match(decision, /principal:\s*PrincipalSummary/u);
});

test('Delivery 1944 protocol adapters authenticate through the frozen factory before constructing envelopes', () => {
  const activation = read(
    'api/apps/api-server/src/extension_bus/authentication_activation.rs',
  );
  const boot = read('api/apps/api-server/src/extension_bus/boot_snapshot.rs');
  const routes = [
    'api/apps/api-server/src/routes/identity/auth.rs',
    'api/apps/api-server/src/routes/application_public_api/native.rs',
    'api/apps/api-server/src/routes/mcp_protocol.rs',
    'api/apps/api-server/src/routes/settings/host_infrastructure/interface_operation.rs',
  ].map(read).join('\n');
  const runtime = productionRustSources(path.join(runtimeRoot, 'src'));

  assert.match(activation, /factory\.authenticate\(Box::new\(credential\)\)\.await/u);
  assert.match(activation, /HostExtensionAuthenticationFactoryCatalog/u);
  assert.match(boot, /authentication_factories\s*\.validate_registry\(&candidate\)/u);
  assert.doesNotMatch(routes, /establish_principal/u);
  assert.match(routes, /\.authenticate_invocation/u);
  assert.doesNotMatch(routes, /\.authenticate\(/u);
  assert.match(routes, /\.into_envelope\(/u);
  assert.doesNotMatch(
    runtime,
    /axum::http::HeaderMap|bearer_token|session_secret|ApplicationApiKeyAuthenticationCredential|McpUserApiKeyAuthenticationCredential/u,
  );
});

test('Delivery 1944 typed production handlers do not import request or host capabilities', () => {
  const handlers = [
    [
      'api/apps/api-server/src/routes/identity/login_entries_interface.rs',
      'PublicLoginEntriesHandler',
      'PublicLoginEntriesAuthorization',
    ],
    [
      'api/apps/api-server/src/routes/application_public_api/native_interface.rs',
      'ApplicationNativeRunHandler',
      'ApplicationNativeRunAuthorization',
    ],
    [
      'api/apps/api-server/src/routes/mcp_protocol/interface_operation.rs',
      'McpInvocationHandler',
      'McpInvocationAuthorization',
    ],
  ];
  for (const [file, startProbe, endProbe] of handlers) {
    const source = read(file);
    const start = source.indexOf(startProbe);
    const end = source.indexOf(endProbe, start);
    assert.ok(start >= 0 && end > start, file);
    const handler = source.slice(start, end);
    assert.doesNotMatch(
      handler,
      /HeaderMap|Cookie|bearer_token|ApiState|Store|Registry|RuntimeHost/u,
    );
  }
});

test('Delivery 1917 binds one graph-frozen hook plan and commits facts through durable outbox', () => {
  const boot = read('api/apps/api-server/src/extension_bus/boot_snapshot.rs');
  const operation = read(
    'api/apps/api-server/src/routes/settings/host_infrastructure/interface_operation.rs',
  );
  const transaction = read(
    'api/crates/storage/durable/postgres/src/model_definition_repository/create.rs',
  );
  const migration = read(
    'api/crates/storage/durable/postgres/migrations/20260828100000_create_lifecycle_outbox.sql',
  );
  const runtimeCargo = read('api/crates/interface-runtime/Cargo.toml');
  const kernel = productionRustSources(path.join(runtimeRoot, 'src/invocation.rs'));

  assert.match(boot, /compile_complete_console_snapshot/u);
  assert.match(read('api/apps/api-server/src/routes/console_interface.rs'), /\.invoke::<I, O, ConsoleInterfaceTargetError>/u);
  assert.doesNotMatch(operation, /invoke_with_hook_plan/u);
  assert.doesNotMatch(kernel, /pub async fn invoke_with_hook_plan/u);
  assert.match(transaction, /record_lifecycle_fact_in_transaction\(&mut tx/u);
  assert.match(transaction, /tx\.commit\(\)\.await/u);
  assert.ok(
    transaction.indexOf('record_lifecycle_fact_in_transaction(&mut tx') <
      transaction.indexOf('tx.commit().await'),
  );
  assert.match(migration, /create table if not exists lifecycle_outbox/u);
  assert.doesNotMatch(migration, /\b(?:drop|alter)\s+(?:table|column)\b/iu);
  assert.doesNotMatch(runtimeCargo, /plugin-framework|extension-contracts|storage-durable/u);
});

test('retired provider configuration is absent while observation uses the compiled Console kernel', () => {
  const boot = read('api/apps/api-server/src/extension_bus/boot_snapshot.rs');
  const http = read('api/apps/api-server/src/routes/settings/host_infrastructure.rs');
  const consoleInterface = read('api/apps/api-server/src/routes/console_interface.rs');
  const contributions = [
    'api/apps/api-server/src/extension_bus/interface_contributions.rs',
    'api/apps/api-server/src/extension_bus/interface_contributions/provider_operations.rs',
    'api/apps/api-server/src/extension_bus/interface_contributions/network.rs',
    'api/apps/api-server/src/extension_bus/interface_contributions/plugins.rs',
    'api/apps/api-server/src/extension_bus/interface_contributions/mcp.rs',
    'api/apps/api-server/src/extension_bus/interface_contributions/system.rs',
  ].map(read).join('\n');
  assert.doesNotMatch(boot + http + contributions, /invoke_providers_view|providers_view_query|interface_provider_config/u);
  assert.match(http, /get_host_infrastructure_memory_overview/u);
  assert.match(http, /console_interface::invoke/u);
  assert.match(consoleInterface, /authenticate_invocation/u);
  assert.match(consoleInterface, /authenticated\.into_envelope/u);
  assert.match(boot, /compile_complete_console_snapshot/u);
});


test('production boundary follows private splits and excludes deliberate test fixtures', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'interface-boundary-'));
  try {
    fs.mkdirSync(path.join(root, 'invocation', 'nested'), { recursive: true });
    fs.mkdirSync(path.join(root, 'invocation', '_tests'));
    fs.writeFileSync(path.join(root, 'invocation.rs'), 'mod nested;');
    fs.writeFileSync(path.join(root, 'invocation', 'nested', 'adapter.rs'), 'use axum::http::HeaderMap;');
    const source = () => productionRustSources(path.join(root, 'invocation.rs'));
    assert.deepEqual(forbiddenSourceImports(source()), ['axum::']);
    fs.writeFileSync(path.join(root, 'invocation', 'nested', 'adapter.rs'), 'use domain::ActorContext;');
    fs.writeFileSync(path.join(root, 'invocation', '_tests', 'negative.rs'), 'use axum::http::HeaderMap;');
    fs.writeFileSync(path.join(root, 'invocation', 'tests.rs'), 'use axum::http::HeaderMap;');
    assert.deepEqual(forbiddenSourceImports(source()), []);
    assert.match(source(), /domain::ActorContext/u);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
