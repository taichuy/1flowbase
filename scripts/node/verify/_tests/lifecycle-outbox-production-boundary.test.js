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
  assert.match(delivery, /fact\.graph_fingerprint != self\.graph_fingerprint/u);
  assert.match(delivery, /handler\.handle\(fact\)\.await/u);
  assert.match(delivery, /ModelDefinitionCommittedFact/u);
  assert.doesNotMatch(delivery, /delivered durable lifecycle fact/u);
  assert.match(dispatcher, /delivery\.deliver\(&fact\)\.await/u);
  assert.match(dispatcher, /&fact\.subscriber_id/u);
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
