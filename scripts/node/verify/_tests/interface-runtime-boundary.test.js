'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const repoRoot = path.resolve(__dirname, '../../../..');
const runtimeRoot = path.join(repoRoot, 'api/crates/interface-runtime');

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');
}

function productionRustSources(root) {
  const sources = [];
  const visit = (directory) => {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      if (entry.name === '_tests' || entry.name === 'tests' || entry.name === 'target') continue;
      const candidate = path.join(directory, entry.name);
      if (entry.isDirectory()) visit(candidate);
      else if (entry.isFile() && path.extname(entry.name) === '.rs') {
        sources.push(fs.readFileSync(candidate, 'utf8'));
      }
    }
  };
  visit(root);
  return sources.join('\n');
}

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

  assert.deepEqual(dependencies, ['domain', 'sha2', 'thiserror', 'tokio', 'uuid']);
  assert.deepEqual(forbiddenSourceImports(production), []);
  assert.deepEqual(forbiddenSourceImports('use axum::http::HeaderMap;'), ['axum::']);
  assert.doesNotMatch(production, /(?:Cookie|HeaderMap|Session|ApiKey)Credential/u);
});

test('Delivery 1944 exposes the approved typed interface facade without infrastructure capabilities', () => {
  const facade = read('api/crates/interface-runtime/src/lib.rs');
  const declaredModules = [...facade.matchAll(/^mod\s+([a-z_]+);$/gmu)].map((match) => match[1]);
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
    'InvocationEnvelope',
    'PrincipalSummary',
    'ProtocolBinding',
    'PublicPrincipal',
    'TypedInterfaceAdmissionPlan',
    'TypedInterfaceAuthorizationPlan',
    'TypedInterfaceDefinitionContribution',
    'UserPrincipal',
  ].sort();

  assert.deepEqual(declaredModules, [
    'authentication',
    'contribution',
    'decision',
    'extension',
    'hook',
    'identity',
    'invocation',
    'principal',
    'registry',
    'stream',
    '_tests',
  ]);
  assert.deepEqual(publicModules, []);
  for (const symbol of requiredSymbols) assert.ok(publicSymbols.includes(symbol), symbol);
  assert.doesNotMatch(facade, /serde_json|axum|sqlx|RegistryHandle|HttpHandler/u);
  assert.match(facade, /pub use invocation::\{/u);
  assert.match(facade, /pub use hook::\{/u);
  assert.match(facade, /pub use registry::\{/u);
});

test('Delivery 1944 routes cannot inject decision or hook plans and plugin decisions only see bounded facts', () => {
  const kernel = read('api/crates/interface-runtime/src/invocation.rs');
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
  assert.match(routes, /\.authenticate\(/u);
  assert.doesNotMatch(
    runtime,
    /axum::http::HeaderMap|bearer_token|session_secret|ApplicationApiKeyAuthenticationCredential|McpUserApiKeyAuthenticationCredential/u,
  );
});

test('Delivery 1944 typed production handlers do not import request or host capabilities', () => {
  const handlers = [
    [
      'api/apps/api-server/src/routes/identity/login_instances_interface.rs',
      'PublicLoginInstancesHandler',
      'PublicLoginInstancesAuthorization',
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
    [
      'api/apps/api-server/src/routes/settings/host_infrastructure/interface_operation.rs',
      'HostInfrastructureProvidersViewHandler',
      'ConsoleInterfaceAuthorizationPort',
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
  const kernel = read('api/crates/interface-runtime/src/invocation.rs');

  assert.match(boot, /compile_hook_plans/u);
  assert.match(boot, /TypedInterfaceHookPlan::new/u);
  assert.match(operation, /compiler\.bind_hook_plan/u);
  assert.match(operation, /\.invoke::<\s*HostInfrastructureProvidersViewInput/u);
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

test('Delivery 1912 production slice consumes one compiled registry for HTTP and MCP', () => {
  const boot = read('api/apps/api-server/src/extension_bus/boot_snapshot.rs');
  const composition = read('api/apps/api-server/src/lib.rs');
  const http = read('api/apps/api-server/src/routes/settings/host_infrastructure.rs');
  const permissionMiddleware = read(
    'api/apps/api-server/src/middleware/require_settings_feature_permission.rs',
  );
  const operation = read(
    'api/apps/api-server/src/routes/settings/host_infrastructure/interface_operation.rs',
  );
  const mcp = read('api/apps/api-server/src/routes/settings/mcp_management/debug_execute.rs');

  assert.match(boot, /DynamicInterfaceRegistry/u);
  assert.match(boot, /compile_interface_registry/u);
  assert.match(composition, /interface_registry\(\)/u);
  assert.match(http, /invoke_providers_view/u);
  assert.match(mcp, /invoke_providers_view/u);
  assert.match(operation, /registry\.snapshot\(\)/u);
  assert.match(operation, /InvocationEnvelope::with_principal/u);
  assert.match(operation, /InterfaceProtocol/u);
  assert.doesNotMatch(operation, /require_session|Cookie|HeaderMap/u);
  const contractDeclarations = [
    ...operation.matchAll(
      /pub struct\s+HostInfrastructureProvidersView(?:Input|Output)(?:\s*\{[^}]*\}|\s*;)/gu,
    ),
  ].map((match) => match[0]);
  const forbiddenContractCapabilities = (source) =>
    ['ApiState', 'MainDurableStore', 'HostInfrastructureRegistry', 'Store', 'Registry'].filter(
      (capability) => source.includes(capability),
    );
  assert.equal(contractDeclarations.length, 2);
  assert.deepEqual(contractDeclarations.flatMap(forbiddenContractCapabilities), []);
  assert.deepEqual(
    forbiddenContractCapabilities(
      'pub struct HostInfrastructureProvidersViewInput { state: Arc<ApiState> }',
    ),
    ['ApiState'],
  );
  assert.match(operation, /HostInfrastructureProvidersViewHandler\s*\{[\s\S]*query/u);
  assert.match(permissionMiddleware, /is_active_interface_route/u);
  assert.ok(
    permissionMiddleware.indexOf('is_active_interface_route') <
      permissionMiddleware.indexOf('require_session(&state, request.headers())'),
  );
  assert.doesNotMatch(permissionMiddleware, /insert\(context\.interface_principal\(\)\)/u);
  assert.match(
    http,
    /list_host_infrastructure_providers\([\s\S]*headers: HeaderMap[\s\S]*ConsoleAuthenticationCredential::Protocol/u,
  );
  assert.doesNotMatch(
    http,
    /Extension\(principal\): Extension<interface_runtime::UserPrincipal>/u,
  );
  assert.doesNotMatch(boot, /InterfaceOperationCatalog/u);
});
