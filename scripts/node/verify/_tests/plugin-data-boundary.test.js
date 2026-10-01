'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const repoRoot = path.resolve(__dirname, '../../../..');
const read = (relative) => fs.readFileSync(path.join(repoRoot, relative), 'utf8');

test('Delivery 1919 keeps PluginData typed and free of implementation capabilities', () => {
  const contract = read('api/crates/extension-contracts/src/plugin_data_contract.rs');
  assert.match(contract, /pub trait PluginDataPort/u);
  assert.match(contract, /enum PluginDataOperation/u);
  assert.match(contract, /enum PluginDataTarget/u);
  assert.doesNotMatch(
    contract,
    /raw_sql|database_url|PgPool|DbConnection|PathBuf|RuntimeExtensionHost|Registry/u,
  );
  const workerFrame = contract.match(/pub enum RuntimeHostWorkerFrame \{([\s\S]*?)\n\}/u);
  assert.ok(workerFrame);
  assert.doesNotMatch(workerFrame[1], /plugin_id|workspace_id|actor_id|publisher_namespace/u);
});

test('Delivery 1919 keeps Host Service outside RuntimeBackend and storage outside Host', () => {
  const backend = read('api/crates/runtime-core/src/runtime_backend.rs');
  const hostCargo = read('api/crates/runtime-extension-host/Cargo.toml');
  const host = read('api/crates/runtime-extension-host/src/runtime_host.rs');
  const boot = read('api/apps/api-server/src/lib.rs');
  assert.doesNotMatch(backend, /PluginDataPort/u);
  assert.doesNotMatch(hostCargo, /storage-durable|control-plane/u);
  assert.match(host, /plugin_data: Arc<dyn PluginDataPort>/u);
  assertPluginDataBoot(boot);
});

// Check the actual injection call, not an unrelated store clone elsewhere in boot.
function assertPluginDataBoot(boot) {
  const call = boot.match(
    /RuntimeExtensionHost::new_with_artifact_resolver_plugin_data_and_profile_source\(\s*process_started_at,\s*runtime_artifact_resolver,\s*Arc::new\(store\.clone\(\)\),\s*Arc::clone\(&runtime_sample_source\),\s*\)/u,
  );
  assert.ok(call, 'composition root must inject PluginData and the shared profile source');
}

test('PluginData boot gate rejects missing injection even with unrelated store clones', () => {
  const boot = read('api/apps/api-server/src/lib.rs');
  assert.throws(() => assertPluginDataBoot(boot.replace(
    '            Arc::new(store.clone()),',
    '            Arc::new(MissingPluginDataPort),',
  )), /composition root must inject PluginData/u);
  assert.throws(() => assertPluginDataBoot(boot.replace(
    '            Arc::clone(&runtime_sample_source),',
    '            Arc::new(RuntimeSampleSource::default()),',
  )), /composition root must inject PluginData/u);
});

test('Delivery 1919 SDK has a contract-only internal dependency graph', () => {
  const cargo = read('api/crates/runtime-extension-sdk/Cargo.toml');
  assert.match(cargo, /extension-contracts/u);
  assert.doesNotMatch(
    cargo,
    /plugin-framework|runtime-extension-host|api-server|control-plane|storage-|runtime-core/u,
  );
  const source = [
    'api/crates/runtime-extension-sdk/src/lib.rs',
    'api/crates/runtime-extension-sdk/src/plugin_data.rs',
    'api/crates/runtime-extension-sdk/src/simulator.rs',
  ].map(read).join('\n');
  assert.doesNotMatch(source, /PgPool|sqlx|PathBuf|RuntimeExtensionHost|ProviderHost|Registry/u);
});

test('Delivery 1919 preserves additive provider wire and trusted internal principal', () => {
  const provider = read('api/crates/runtime-extension-host/src/provider_host.rs');
  const stdio = read('api/crates/runtime-extension-host/src/stdio_runtime.rs');
  const apiRuntime = read('api/apps/api-server/src/provider_runtime/mod.rs');
  assert.match(provider, /current_provider_wire_input\(&loaded, &input\)/u);
  assert.match(stdio, /RuntimeHostWorkerFrame/u);
  assert.match(stdio, /ProviderRuntimeLine/u);
  assert.match(apiRuntime, /RuntimeExecutionPrincipal/u);
  assert.doesNotMatch(
    read('api/crates/extension-contracts/src/provider_contract.rs'),
    /RuntimeExecutionPrincipal|workspace_id.*actor_id/u,
  );
});
