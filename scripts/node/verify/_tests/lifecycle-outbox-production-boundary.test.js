import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const root = path.resolve(import.meta.dirname, '../../../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');

test('production boot starts the durable lifecycle dispatcher', () => {
  const api = read('api/apps/api-server/src/lib.rs');
  assert.match(api, /LifecycleOutboxDispatcher::new/u);
  assert.match(api, /ApiLifecycleFactDelivery/u);
  assert.match(api, /ApiLifecycleDeliveryCompletion/u);
  assert.match(api, /compile_lifecycle_subscriber_plan/u);
  assert.match(api, /with_lifecycle_publication_catalog/u);
});

test('claim query recovers stale workers through a bounded lease', () => {
  const repository = read(
    'api/crates/storage/durable/postgres/src/lifecycle_outbox_repository.rs'
  );
  assert.match(repository, /claim_lease/u);
  assert.match(repository, /status = 'claimed' and claimed_at <= \$3/u);
  assert.match(repository, /attempt_count = attempt_count \+ 1/u);
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
  assert.match(dispatcher, /delivery\.deliver\(&fact\)\.await/u);
  assert.match(dispatcher, /&fact\.subscriber_id/u);
});

test('composition injects one activated typed handler registry', () => {
  const api = read('api/apps/api-server/src/lib.rs');
  const delivery = read('api/apps/api-server/src/host_extensions/lifecycle.rs');
  const registry = read(
    'api/crates/plugin-framework/src/extension_bus/lifecycle_handler_registry.rs'
  );
  assert.match(api, /builtin_lifecycle_handler_bindings/u);
  assert.match(delivery, /LifecycleHandlerBinding::typed/u);
  assert.match(registry, /TypedLifecycleSubscriberHandler/u);
  assert.match(registry, /invalid typed lifecycle fact/u);
  assert.doesNotMatch(registry, /serde_json::Value/u);
});

test('lifecycle contribution privileges are explicit for all plugin kinds', () => {
  const plan = read(
    'api/crates/plugin-framework/src/extension_bus/lifecycle_subscriber_plan.rs'
  );
  assert.match(plan, /ModuleKind::TrustedHost/u);
  assert.match(plan, /ModuleKind::Runtime/u);
  assert.match(plan, /ModuleKind::Capability/u);
  assert.match(plan, /LifecycleEscalation/u);
});

test('outbox records per-subscriber durable state and aggregate completion', () => {
  const migration = read(
    'api/crates/storage/durable/postgres/migrations/20260828190000_add_lifecycle_subscriber_deliveries.sql'
  );
  const repository = read(
    'api/crates/storage/durable/postgres/src/lifecycle_outbox_repository.rs'
  );
  assert.match(migration, /primary key \(event_id, subscriber_id\)/u);
  assert.match(repository, /lifecycle_outbox_deliveries/u);
  assert.match(repository, /status <> 'delivered'/u);
  assert.match(repository, /different publication plan/u);
});

test('dispatcher emits a typed completion on the production delivery path', () => {
  const dispatcher = read(
    'api/crates/control-plane/src/lifecycle_outbox_dispatcher.rs'
  );
  assert.match(dispatcher, /CompletionOutcome::new/u);
  assert.match(dispatcher, /mark_lifecycle_fact_delivered/u);
  assert.match(dispatcher, /retry_lifecycle_fact/u);
});
