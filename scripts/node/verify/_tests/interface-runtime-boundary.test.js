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

test('Delivery 1912 keeps interface-runtime dependency closed and AuthN adapter-owned', () => {
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

test('Delivery 1912 exposes only the approved typed interface facade', () => {
  const facade = read('api/crates/interface-runtime/src/lib.rs');
  const declaredModules = [...facade.matchAll(/^mod\s+([a-z_]+);$/gmu)].map((match) => match[1]);
  const publicModules = [...facade.matchAll(/^pub\s+mod\s+([a-z_]+);$/gmu)].map(
    (match) => match[1],
  );
  const publicSymbols = [...facade.matchAll(/pub use [a-z_]+::\{([\s\S]*?)\};/gu)]
    .flatMap((match) => match[1].split(',').map((symbol) => symbol.trim()).filter(Boolean))
    .sort();
  const approvedSymbols = [
    'CompiledInterfaceRegistry',
    'ContractIdentity',
    'DynamicInterfaceRegistry',
    'GraphFingerprint',
    'HandlerReference',
    'IdentityError',
    'InterfaceAuditPolicy',
    'InterfaceAuthenticationPolicy',
    'InterfaceAuthorizationError',
    'InterfaceAuthorizationFuture',
    'InterfaceAuthorizationPort',
    'InterfaceAuthorizationRequest',
    'InterfaceContract',
    'InterfaceDefinition',
    'InterfaceErrorPolicy',
    'InterfaceHandler',
    'InterfaceHandlerContext',
    'InterfaceHandlerFuture',
    'InterfaceId',
    'InterfaceInvocationError',
    'InterfaceInvocationFailure',
    'InterfaceInvocationKernel',
    'InterfaceInvocationOutcome',
    'InterfaceInvocationReceipt',
    'InterfaceInvocationResult',
    'InterfaceInvocationStage',
    'InterfaceInvocationTerminal',
    'InterfaceLifecycle',
    'InterfaceOwner',
    'InterfaceProtocol',
    'InterfaceScope',
    'InterfaceTargetAdmissionError',
    'InterfaceTargetAdmissionFuture',
    'InterfaceTargetAdmissionPort',
    'InterfaceTargetAdmissionRequest',
    'InterfaceTargetError',
    'InvocationEnvelope',
    'InvocationId',
    'InvocationLineage',
    'InvocationLineageError',
    'PermissionIdentity',
    'RegistryCompilationError',
    'RegistryCompiler',
    'RegistryFingerprint',
    'RouteIdentity',
    'TargetReference',
  ].sort();

  assert.deepEqual(declaredModules, ['identity', 'invocation', 'registry', '_tests']);
  assert.deepEqual(publicModules, []);
  assert.deepEqual(publicSymbols, approvedSymbols);
  assert.doesNotMatch(facade, /serde_json|axum|sqlx|RegistryHandle|HttpHandler/u);
  assert.match(facade, /pub use invocation::\{/u);
  assert.match(facade, /pub use registry::\{/u);
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
  assert.match(operation, /InvocationEnvelope::new/u);
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
  assert.match(permissionMiddleware, /extensions_mut\(\)\.insert\(context\.actor\)/u);
  assert.match(
    http,
    /list_host_infrastructure_providers\([\s\S]*Extension\(actor\): Extension<domain::ActorContext>/u,
  );
  assert.doesNotMatch(boot, /InterfaceOperationCatalog/u);
});
