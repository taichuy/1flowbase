import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { bootRustSources } from '../../testing/contracts/rust-sources.js';

const root = path.resolve(import.meta.dirname, '../../../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');

test('production boot starts the durable lifecycle dispatcher', () => {
  const api = bootRustSources(root);
  assert.match(api, /LifecycleOutboxDispatcher::new/u);
  assert.match(api, /ApiLifecycleFactDelivery/u);
  assert.match(api, /ApiLifecycleDeliveryCompletion/u);
  assert.match(api, /compile_lifecycle_subscriber_plan/u);
  assert.match(api, /with_lifecycle_publication_catalog/u);
});

test('durable delivery invokes frozen typed subscribers before acknowledgement', () => {
  const delivery = read(
    'api/apps/api-server/src/host_extensions/lifecycle.rs'
  );
  const dispatcher = read(
    'api/crates/control-plane/src/lifecycle_outbox_dispatcher.rs'
  );
  assert.match(delivery, /compile_lifecycle_handler_registry/u);
  assert.match(delivery, /self\.registry[\s\S]*?\.deliver\(/u);
  assert.match(delivery, /ModelDefinitionCommittedFact/u);
  assert.doesNotMatch(delivery, /match \(handler_id, handler_version\)/u);
  assert.doesNotMatch(delivery, /match \([\s\S]*?subscriber\.handler_id/u);
  assert.doesNotMatch(delivery, /delivered durable lifecycle fact/u);
  assert.match(dispatcher, /tokio::time::timeout\(/u);
  assert.match(dispatcher, /self\.delivery\.deliver\(&fact\)/u);
  assert.match(dispatcher, /&fact\.subscriber_id/u);
});

test('composition resolves active HostExtension entrypoint factories before registry binding', () => {
  const api = bootRustSources(root);
  const delivery = read('api/apps/api-server/src/host_extensions/lifecycle.rs');
  const activation = read(
    'api/apps/api-server/src/host_extensions/lifecycle_activation.rs'
  );
  const registry = read(
    'api/crates/plugin-framework/src/extension_bus/lifecycle_handler_registry.rs'
  );
  assert.match(api, /extend_active_host_extensions\(prepared_host_extensions\.graph_extensions\(\)\)/u);
  assert.match(api, /production_lifecycle_handler_factories[\s\S]*?\.activate\(&active_host_extensions\)/u);
  assert.doesNotMatch(api, /builtin_lifecycle_handler_bindings/u);
  assert.match(activation, /contribution\.native\.library/u);
  assert.match(activation, /contribution\.native\.entry_symbol/u);
  assert.match(activation, /factory\(\)\?/u);
  assert.match(delivery, /LifecycleHandlerBinding::typed/u);
  assert.match(delivery, /acme_lifecycle_subscriber_fixture/u);
  assert.match(registry, /TypedLifecycleSubscriberHandler/u);
  assert.match(registry, /invalid typed lifecycle fact/u);
  assert.doesNotMatch(registry, /serde_json::Value/u);
});

test('durable lifecycle subscriptions admit only the finite managed lifecycle transport', () => {
  const plan = read(
    'api/crates/plugin-framework/src/extension_bus/lifecycle_subscriber_plan.rs'
  );
  assert.match(plan, /ModuleKind::TrustedHost/u);
  assert.match(plan, /ModuleKind::Runtime \| ModuleKind::Capability => matches!/u);
  assert.match(plan, /LifecycleSemantics::Invocation \| LifecycleSemantics::WorkspaceAssignment/u);
  assert.match(plan, /ModuleKind::User => false/u);
  assert.match(plan, /LifecycleEscalation/u);
});

test('managed resources close and wait before shutdown cleanup without deleting backlog', () => {
  const api = read('api/apps/api-server/src/lib.rs');
  const shutdown = api.slice(api.indexOf('impl ApiRuntimeShutdown'));
  for (const marker of ['impl ApiRuntimeShutdown', 'lifecycle_worker', 'close_owned_admission', 'self.operations.close()', 'wait_for_shutdown', 'self.host.stop()', 'cleanup_after_shutdown']) assert.ok(shutdown.includes(marker), marker);
  assert.ok(shutdown.indexOf('lifecycle_worker') < shutdown.indexOf('close_owned_admission'));
  assert.ok(shutdown.indexOf('self.operations.close()') < shutdown.indexOf('wait_for_shutdown'));
  assert.ok(shutdown.indexOf('self.host.stop()') < shutdown.indexOf('cleanup_after_shutdown'));
  assert.doesNotMatch(shutdown, /delete from|truncate /iu);
  const dispatcher = read('api/crates/control-plane/src/lifecycle_outbox_dispatcher.rs');
  assert.match(dispatcher, /claim_id/u);
  assert.match(dispatcher, /pause_lifecycle_fact/u);
  assert.match(dispatcher, /pub fn close/u);
});
