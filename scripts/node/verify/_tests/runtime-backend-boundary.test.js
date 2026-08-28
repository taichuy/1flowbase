'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const repoRoot = path.resolve(__dirname, '../../../..');
const forbidden = /PLUGIN_RUNNER|plugin-runner|plugin_runner|PluginRunner|\b7801\b/u;
const ignoredDirectories = new Set(['_tests', 'node_modules', 'target', 'volumes']);

function productionFiles(root, ignored = ignoredDirectories) {
  const files = [];
  const visit = (directory) => {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      if (ignored.has(entry.name)) continue;
      const candidate = path.join(directory, entry.name);
      if (entry.isDirectory()) visit(candidate);
      else if (entry.isFile()) files.push(candidate);
    }
  };
  visit(root);
  return files;
}

test('Delivery 1898 removes the standalone runtime service from production tooling', () => {
  const roots = ['scripts', 'docker', '.github'].map((directory) => path.join(repoRoot, directory));
  const violations = roots.flatMap((root) => productionFiles(root)).flatMap((file) => {
    const source = fs.readFileSync(file, 'utf8');
    return forbidden.test(source) ? [path.relative(repoRoot, file)] : [];
  });
  assert.deepEqual(violations, []);
});

test('Delivery 1898 keeps one Backend executable in the Cargo workspace', () => {
  const workspace = fs.readFileSync(path.join(repoRoot, 'api/Cargo.toml'), 'utf8');
  assert.match(workspace, /"apps\/api-server"/u);
  assert.doesNotMatch(workspace, forbidden);
  assert.equal(fs.existsSync(path.join(repoRoot, 'api/apps/plugin-runner')), false);
});

test('Delivery 1898 binds the production Backend Slot and hides concrete registries', () => {
  const boot = fs.readFileSync(path.join(repoRoot, 'api/apps/api-server/src/lib.rs'), 'utf8');
  const runtime = fs.readFileSync(
    path.join(repoRoot, 'api/apps/api-server/src/provider_runtime/mod.rs'),
    'utf8',
  );
  const host = fs.readFileSync(
    path.join(repoRoot, 'api/crates/runtime-extension-host/src/runtime_host.rs'),
    'utf8',
  );
  const contract = fs.readFileSync(
    path.join(repoRoot, 'api/crates/runtime-core/src/runtime_backend.rs'),
    'utf8',
  );

  assert.match(boot, /RuntimeBackendSlot::default\(\)/u);
  assert.match(boot, /runtime_backend_slot\.bind\(runtime_extension_host\.clone\(\)\)/u);
  assert.match(runtime, /orchestration_backend/u);
  assert.doesNotMatch(runtime, /\.(?:provider|data_source|capability|network_egress)_registry\(\)/u);
  assert.doesNotMatch(host, /pub fn (?:provider|data_source|capability|network_egress)_registry/u);
  assert.match(contract, /pub struct RuntimeArtifactReference/u);
  assert.doesNotMatch(contract, /pub package_root:/u);
});

test('Delivery 1898 projects only the execution port into orchestration', () => {
  const boot = fs.readFileSync(path.join(repoRoot, 'api/apps/api-server/src/lib.rs'), 'utf8');
  const runtime = fs.readFileSync(
    path.join(repoRoot, 'api/apps/api-server/src/provider_runtime/mod.rs'),
    'utf8',
  );
  const orchestration = fs.readFileSync(
    path.join(repoRoot, 'api/crates/orchestration-runtime/src/runtime_backend.rs'),
    'utf8',
  );
  const constructorParameters = runtime.match(
    /pub fn new_with_runtime_backend\(([\s\S]*?)\) -> anyhow::Result<Self>/u,
  );

  assert.match(orchestration, /Arc<dyn RuntimeExecutionPort>/u);
  assert.doesNotMatch(orchestration, /\bRuntimeBackend\b/u);
  assert.ok(constructorParameters, 'Runtime Backend constructor signature must be inspectable');
  assert.match(constructorParameters[1], /runtime_backend: Arc<dyn RuntimeBackend>/u);
  assert.match(constructorParameters[1], /extension_graph:/u);
  assert.doesNotMatch(constructorParameters[1], /runtime_execution/u);
  assert.match(
    runtime,
    /let runtime_execution: Arc<dyn RuntimeExecutionPort> = runtime_backend\.clone\(\);/u,
  );
  assert.doesNotMatch(boot, /let runtime_execution:/u);
  assert.match(
    boot,
    /ApiRuntimeServices::new_with_runtime_backend\(\s*runtime_backend,\s*Arc::clone\(&extension_graph\),\s*\)/u,
  );
  assert.equal((boot.match(/runtime_backend_slot\.backend\(\)\?/gu) ?? []).length, 1);
});

test('Delivery 1898 requires all six narrow Runtime Backend ports', () => {
  const contract = fs.readFileSync(
    path.join(repoRoot, 'api/crates/runtime-core/src/runtime_backend.rs'),
    'utf8',
  );
  const ports = [
    'RuntimeExecutionPort',
    'RuntimeObservationPort',
    'ProviderRuntimePort',
    'DataSourceRuntimePort',
    'CapabilityRuntimePort',
    'NetworkEgressRuntimePort',
  ];
  const backendComposition = contract.match(
    /pub trait RuntimeBackend:([\s\S]*?)\n\{/u,
  );
  assert.ok(backendComposition, 'RuntimeBackend composition must be inspectable');

  for (const port of ports) {
    assert.match(contract, new RegExp(`pub trait ${port}`, 'u'));
    assert.match(backendComposition[1], new RegExp(port, 'u'));
  }
  assert.doesNotMatch(contract, /trait RuntimeExtensionPort/u);
  assert.doesNotMatch(contract, /pub package_root:|PathBuf|std::process|http::|grpc/u);

  for (const port of ports.slice(2)) {
    const declaration = contract.match(
      new RegExp(`pub trait ${port}[^\\{]*\\{([\\s\\S]*?)\\n\\}`, 'u'),
    );
    assert.ok(declaration, `${port} declaration must be inspectable`);
    assert.doesNotMatch(declaration[1], /UnsupportedOperation|Err\(/u);
    assert.match(contract, new RegExp(`Incomplete${port.replace('RuntimePort', '')}Backend`, 'u'));
  }

  const fixture = fs.readFileSync(
    path.join(repoRoot, 'api/crates/runtime-core/src/_tests/runtime_backend_tests.rs'),
    'utf8',
  );
  for (const port of ports) {
    assert.match(fixture, new RegExp(`impl ${port} for CompleteFakeBackend`, 'u'));
  }
  assert.match(fixture, /Arc<dyn RuntimeBackend>/u);
});

test('Delivery 1898 exposes only the approved Runtime Host facade', () => {
  const hostRoot = path.join(repoRoot, 'api/crates/runtime-extension-host');
  const facade = fs.readFileSync(path.join(hostRoot, 'src/lib.rs'), 'utf8');
  const cargo = fs.readFileSync(path.join(hostRoot, 'Cargo.toml'), 'utf8');
  const forbiddenModules = [
    'provider_host',
    'data_source_host',
    'capability_host',
    'network_egress_host',
    'stdio_runtime',
    'package_loader',
  ];

  assert.match(
    facade,
    /pub use runtime_host::\{RuntimeArtifactResolver, RuntimeExtensionHost\};/u,
  );
  for (const moduleName of forbiddenModules) {
    assert.doesNotMatch(facade, new RegExp(`pub mod ${moduleName}`, 'u'));
  }
  assert.match(facade, /compile_fail/u);
  assert.doesNotMatch(cargo, /plugin-framework|axum|reqwest/u);

  const forbiddenImport = new RegExp(
    `runtime_extension_host::(?:${forbiddenModules.join('|')})(?:::|\\b)`,
    'u',
  );
  const consumerRoots = [
    path.join(repoRoot, 'api/apps/api-server/src'),
    path.join(repoRoot, 'api/crates/orchestration-runtime/src'),
  ];
  const consumerIgnoredDirectories = new Set(['node_modules', 'target']);
  const violations = consumerRoots
    .flatMap((root) => productionFiles(root, consumerIgnoredDirectories))
    .flatMap((file) => {
      if (path.extname(file) !== '.rs') return [];
      return forbiddenImport.test(fs.readFileSync(file, 'utf8'))
        ? [path.relative(repoRoot, file)]
        : [];
    });
  assert.deepEqual(violations, []);
});

test('Delivery 1920 keeps provider distribution inside the execution port and routing owner', () => {
  const contract = fs.readFileSync(
    path.join(repoRoot, 'api/crates/runtime-core/src/runtime_backend.rs'),
    'utf8',
  );
  const orchestration = fs.readFileSync(
    path.join(repoRoot, 'api/crates/orchestration-runtime/src/execution_engine/provider_routing.rs'),
    'utf8',
  );
  const hostFacade = fs.readFileSync(
    path.join(repoRoot, 'api/crates/runtime-extension-host/src/lib.rs'),
    'utf8',
  );
  const migration = fs.readFileSync(
    path.join(
      repoRoot,
      'api/crates/storage/durable/postgres/migrations/20260828160000_open_provider_distribution_rule_identity.sql',
    ),
    'utf8',
  );

  assert.match(contract, /trait RuntimeExecutionPort[\s\S]*select_provider_distribution/u);
  assert.doesNotMatch(contract, /trait ProviderDistributionRuntimePort/u);
  assert.match(orchestration, /select_provider_distribution/u);
  assert.match(orchestration, /selected an ineligible target/u);
  assert.doesNotMatch(hostFacade, /pub mod provider_distribution_host/u);
  assert.match(migration, /drop constraint if exists model_provider_main_model_distribution_rules_rule_check/u);
  assert.doesNotMatch(migration, /check \(distribution_rule in/u);
  assert.doesNotMatch(
    migration,
    /set\s+distribution_rule\s*=\s*'builtin\.'\s*\|\|\s*distribution_rule/u,
    'additive identity support must not rewrite values needed by the rollback binary',
  );
});
