import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const root = path.resolve(import.meta.dirname, '../../../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');

test('production composition compiles and owns the effective distribution snapshot', () => {
  const registry = read(
    'api/apps/api-server/src/provider_runtime/distribution_registry.rs'
  );
  const runtime = read('api/apps/api-server/src/provider_runtime/mod.rs');
  assert.match(registry, /ProviderDistributionRuleRegistry::compile/u);
  assert.match(runtime, /provider_distribution_snapshot/u);
  assert.match(runtime, /with_runtime_package/u);
  assert.match(runtime, /resolve_runtime/u);
});

test('API validation and catalog project the effective distribution snapshot', () => {
  const route = read(
    'api/apps/api-server/src/routes/plugins_and_models/model_providers.rs'
  );
  assert.match(route, /provider_distribution_definitions\(\)[\s\S]*?\.await/u);
  assert.match(route, /validate_provider_distribution_rule/u);
});

test('orchestration freezes a production registry fingerprint and has no fixed dynamic fingerprint', () => {
  const routing = read(
    'api/crates/orchestration-runtime/src/execution_engine/provider_routing.rs'
  );
  const context = read(
    'api/crates/orchestration-runtime/src/execution_engine/run_input.rs'
  );
  assert.match(routing, /provider_distribution_registry_fingerprint\(invoker\)/u);
  assert.match(context, /OnceCell<String>/u);
  assert.doesNotMatch(routing, /BUILTIN_REGISTRY_FINGERPRINT/u);
});

test('model provider UI renders rule options from the API catalog', () => {
  const modal = read(
    'web/app/src/features/settings/components/model-providers/ModelProviderRoutingPolicyModal.tsx'
  );
  assert.match(modal, /distributionRules\.map/u);
  assert.doesNotMatch(modal, /<option value="none">/u);
  assert.doesNotMatch(modal, /<option value="round_robin">/u);
});
