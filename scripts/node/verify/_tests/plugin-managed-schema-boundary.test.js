const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');
const { productionRustSources } = require('../../testing/contracts/rust-sources.js');

const root = path.resolve(__dirname, '../../../..');
const read = (relative) => productionRustSources(path.join(root, relative));

test('PDM boundary keeps manifest as the single additive declaration', () => {
  const manifest = [
    'api/crates/extension-package-runtime/src/manifest_v1.rs',
    'api/crates/extension-package-runtime/src/manifest_v1/frontend_blocks.rs',
  ].map(read).join('\n');
  const compiler = read('api/crates/plugin-framework/src/managed_schema/mod.rs');
  const adapter = read('api/crates/storage/durable/postgres/src/managed_schema_repository.rs');

  assert.match(manifest, /pub data_models: Vec<crate::PluginDataModelContribution>/);
  assert.doesNotMatch(`${manifest}\n${compiler}`, /extension_field_slot/);
  assert.doesNotMatch(adapter, /drop\s+(table|column)|rename\s+column|alter\s+column/i);
});

test('PDM production path consumes compiler, preview, apply, and retain lifecycle', () => {
  const composition = read(
    'api/apps/api-server/src/routes/plugins_and_models/plugins/extension_center/managed_schema.rs',
  );
  const routes = read(
    'api/apps/api-server/src/routes/plugins_and_models/plugins/extension_center.rs',
  );
  const upload = read(
    'api/apps/api-server/src/routes/plugins_and_models/plugins/extension_center/upload.rs',
  );

  const service = read(
    'api/crates/control-plane/src/plugin_management/managed_activation/schema.rs',
  );
  assert.match(composition, /control_plane::plugin_management::prepare_managed_schema_plan\(/);
  assert.doesNotMatch(composition, /compile_managed_schema_plan\(/);
  assert.match(service, /compile_managed_schema_plan\(/);
  assert.match(service, /repository\.preview_managed_schema\(&plan\)/);
  assert.match(composition, /apply_managed_schema/);
  assert.match(routes, /retain_managed_schema/);
  assert.match(upload, /prepare_managed_schema/);
});

test('PDM ports do not expose SQL, connection, or runtime transport details', () => {
  const contract = read('api/crates/control-plane-contracts/src/ports/extension_ports/managed_schema.rs');
  assert.doesNotMatch(contract, /sqlx|PgPool|Connection|raw_sql|local_path|stdio|http|grpc/i);
});
