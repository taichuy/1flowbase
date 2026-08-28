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
});

test('claim query recovers stale workers through a bounded lease', () => {
  const repository = read(
    'api/crates/storage/durable/postgres/src/lifecycle_outbox_repository.rs'
  );
  assert.match(repository, /claim_lease/u);
  assert.match(repository, /status = 'claimed' and claimed_at <= \$3/u);
  assert.match(repository, /attempt_count = attempt_count \+ 1/u);
});

test('dispatcher emits a typed completion on the production delivery path', () => {
  const dispatcher = read(
    'api/crates/control-plane/src/lifecycle_outbox_dispatcher.rs'
  );
  assert.match(dispatcher, /CompletionOutcome::new/u);
  assert.match(dispatcher, /mark_lifecycle_fact_delivered/u);
  assert.match(dispatcher, /retry_lifecycle_fact/u);
});
