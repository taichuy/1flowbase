const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  buildContractReceipt,
  buildFoundationPlan,
  buildQualityGateComponentReport,
  validatePackInventory,
  writeQualityGateComponentArtifacts,
} = require('../core.js');

test('AC-001/006 routes four foundations and ignores legal non-contract changes', () => {
  const matrix = [
    ['ai-gateway', 'scripts/node/ai-gateway-concurrency/contracts/index.js'],
    ['mcp-gateway', 'api/apps/api-server/src/routes/mcp_protocol.rs'],
    ['application-backend', 'api/apps/api-server/src/_tests/application/model_definition_routes/model_crud.rs'],
    ['native-react', 'web/packages/page-runtime/src/native-react-compiler/source-contract.ts'],
  ];

  for (const [foundation, changedFile] of matrix) {
    const plan = buildFoundationPlan({ changedFiles: [changedFile] });
    assert.deepEqual(plan.selectedFoundations, [foundation]);
  }

  for (const nativeReactHostContractFile of [
    'web/app/package.json',
    'web/pnpm-lock.yaml',
    'web/app/src/features/frontstage/lib/native-modules/registry.ts',
  ]) {
    const plan = buildFoundationPlan({ changedFiles: [nativeReactHostContractFile] });
    assert.deepEqual(plan.selectedFoundations, ['native-react']);
  }

  const legalPlan = buildFoundationPlan({
    changedFiles: [
      'docs/quality-gates.md',
      'web/app/src/features/frontstage/styles/layout.css',
      'web/app/src/i18n/zh-CN.json',
      'README.md',
    ],
  });
  assert.deepEqual(legalPlan.selectedFoundations, []);

  const providerSettingsPlan = buildFoundationPlan({
    changedFiles: ['api/apps/api-server/src/routes/plugins_and_models/model_providers/dto.rs'],
  });
  assert.equal(providerSettingsPlan.selectedFoundations.includes('application-backend'), false);
});

test('routes current protocol and invocation owners without selecting unrelated settings', () => {
  for (const changedFile of [
    'api/apps/api-server/src/routes/application_public_api/anthropic.rs',
    'api/apps/api-server/src/routes/application_public_api/anthropic/error_projection.rs',
    'api/apps/api-server/src/routes/application_public_api/anthropic/_tests/error_projection.rs',
    'api/apps/api-server/src/routes/application_public_api/client_observer.rs',
    'api/apps/api-server/src/routes/application_public_api/openai.rs',
    'api/apps/api-server/src/routes/application_public_api/openai/chat_completions.rs',
    'api/apps/api-server/src/routes/application_public_api/compatibility_interface.rs',
    'api/crates/control-plane/src/application_public_api/client_stream.rs',
    'api/crates/control-plane/src/client_trajectory/chat.rs',
    'api/crates/control-plane/src/orchestration_runtime/provider_invoker.rs',
    'api/crates/control-plane/src/orchestration_runtime/provider_invoker/stream.rs',
  ]) {
    assert.deepEqual(buildFoundationPlan({ changedFiles: [changedFile] }).selectedFoundations, ['ai-gateway'], changedFile);
  }
  for (const changedFile of [
    'api/crates/control-plane/src/settings/mod.rs',
    'api/apps/api-server/src/routes/application_public_api/anthropic_settings.rs',
    'api/apps/api-server/src/routes/application_public_api/client_observer_settings.rs',
    'api/apps/api-server/src/routes/plugins_and_models/model_providers/dto.rs',
    'docs/architecture/interface-lifecycle.md',
  ]) {
    assert.deepEqual(buildFoundationPlan({ changedFiles: [changedFile] }).selectedFoundations, [], changedFile);
  }
});

test('routes real native transport and replay owners without widening the AI fast pack', () => {
  const repoRoot = path.resolve(__dirname, '../../../..');
  const baselinePack = buildFoundationPlan({ foundation: 'ai-gateway' }).packs['ai-gateway'];
  for (const changedFile of [
    'api/apps/api-server/src/routes/application_public_api/native.rs',
    'api/apps/api-server/src/routes/application_public_api/native_interface.rs',
    'api/apps/api-server/src/routes/application_public_api/native_read_interface.rs',
    'api/apps/api-server/src/routes/application_public_api/native/_tests/catalog_admission.rs',
    'api/apps/api-server/src/routes/application_public_api/native_websocket/turn_bridge.rs',
    'api/apps/api-server/src/routes/application_public_api/sse.rs',
    'api/apps/api-server/src/routes/application_public_api/stream_terminal_fallback.rs',
    'api/apps/api-server/src/routes/application_public_api/stream_terminal_fallback/cold_replay.rs',
    'api/apps/api-server/src/routes/applications/debug_run_stream.rs',
    'api/apps/api-server/src/host_infrastructure/local_runtime_event_stream.rs',
    'api/apps/api-server/src/provider_runtime/transport_session_lifecycle.rs',
    'api/apps/api-server/src/provider_runtime/_tests/transport_session_lifecycle/dispatch_settlement.rs',
    'api/apps/api-server/src/_tests/runtime_event_stream/recoverable.rs',
    'api/crates/control-plane/src/orchestration_runtime/runtime_event_persister.rs',
    'api/crates/control-plane/src/_tests/orchestration_runtime/runtime_event_persister.rs',
    'api/crates/control-plane/src/orchestration_runtime/callback_completion.rs',
    'api/crates/orchestration-runtime/src/transport_session/registry.rs',
    'api/crates/orchestration-runtime/src/transport_session/_tests/dispatch_settlement.rs',
  ]) {
    assert.equal(fs.existsSync(path.join(repoRoot, changedFile)), true, `current owner must exist: ${changedFile}`);
    const plan = buildFoundationPlan({ changedFiles: [changedFile] });
    assert.deepEqual(plan.selectedFoundations, ['ai-gateway'], changedFile);
    assert.deepEqual(plan.packs['ai-gateway'].triggerReasons, [`changed: ${changedFile}`]);
    assert.deepEqual(plan.packs['ai-gateway'].fast, baselinePack.fast);
    assert.deepEqual(plan.packs['ai-gateway'].full, baselinePack.full);
  }
});

test('native transport routing excludes non-Rust assets and unrelated runtime owners', () => {
  for (const changedFile of [
    'api/apps/api-server/src/routes/application_public_api/native/README.md',
    'api/apps/api-server/src/routes/application_public_api/native/i18n/en_US.json',
    'api/apps/api-server/src/routes/application_public_api/native/styles.css',
    'api/apps/api-server/src/routes/application_public_api/native_settings.rs',
    'api/apps/api-server/src/routes/plugins_and_models/model_providers/dto.rs',
    'api/apps/api-server/src/host_infrastructure/local.rs',
    'api/apps/api-server/src/provider_runtime/README.md',
    'api/crates/control-plane/src/orchestration_runtime/tests/billing_usage_guard.rs',
    'api/crates/control-plane/src/orchestration_runtime/frozen_plan/mod.rs',
    'api/crates/orchestration-runtime/src/transport_session/README.md',
    'api/crates/runtime-core/src/capability_slots.rs',
    'docs/architecture/ai-gateway-observation.md',
    'web/app/src/i18n/zh-CN.json',
    'web/app/src/features/frontstage/styles/layout.css',
  ]) {
    assert.deepEqual(buildFoundationPlan({ changedFiles: [changedFile] }).selectedFoundations, [], changedFile);
  }
});

test('AC-002 keeps mcp_result outside the core operations and only adds continuation evidence on risk', () => {
  assert.doesNotThrow(() => validatePackInventory());

  assert.throws(
    () => validatePackInventory({ mcpCoreOperations: ['mcp_list', 'mcp_get', 'mcp_result', 'mcp_call'] }),
    /mcp_list -> mcp_get -> mcp_call/u,
  );

  const corePlan = buildFoundationPlan({
    changedFiles: ['api/apps/api-server/src/routes/mcp_protocol.rs'],
  });
  assert.equal(corePlan.packs['mcp-gateway'].fast.some((item) => item.id === 'mcp-result-continuation'), false);

  const continuationPlan = buildFoundationPlan({
    changedFiles: ['api/apps/api-server/src/routes/mcp_protocol/result_delivery.rs'],
  });
  assert.equal(
    continuationPlan.packs['mcp-gateway'].fast.some((item) => item.id === 'mcp-result-continuation'),
    true,
  );
});

test('mcp gateway fast pack uses Cargo default test parallelism', () => {
  const plan = buildFoundationPlan({
    changedFiles: ['api/apps/api-server/src/routes/mcp_protocol.rs'],
  });
  const command = plan.packs['mcp-gateway'].fast.find(
    (item) => item.id === 'mcp-core-list-get-call'
  );

  assert.deepEqual(command.args, [
    'test',
    '-p',
    'api-server',
    'mcp_protocol_routes',
  ]);
});

test('AC-007/009 receipt requires candidate identity and warnings stay advisory', () => {
  const plan = buildFoundationPlan({
    changedFiles: ['web/packages/block-sdk/src/native-react.ts'],
  });

  assert.throws(
    () => buildContractReceipt({ candidateSha: '', plan, componentResults: [] }),
    /candidate SHA/u,
  );

  const warningReceipt = buildContractReceipt({
    candidateSha: 'abcdef1234567890',
    plan,
    componentResults: [{
      candidateSha: 'abcdef1234567890',
      foundation: 'native-react',
      status: 'passed',
      exitCode: 0,
      executedPack: plan.packs['native-react'].fast.map((item) => item.id),
      commands: plan.packs['native-react'].fast.map((item) => ({
        id: item.id, exitCode: 0, error: '', passedCount: 1, failedCount: 0,
        logPath: `tmp/test-governance/foundation-contracts/components/native-react/${item.id}.log`,
      })),
      warnings: ['nightly browser matrix deferred'],
      warningFiles: ['tmp/test-governance/native-react.warnings.log'],
      errors: [],
    }],
  });
  assert.equal(warningReceipt.status, 'passed');
  assert.deepEqual(warningReceipt.warningFiles, ['tmp/test-governance/native-react.warnings.log']);
  assert.deepEqual(warningReceipt.warnings, ['nightly browser matrix deferred']);
  assert.ok(warningReceipt.deferredEvidence.length > 0);
  assert.equal(warningReceipt.candidateSha, 'abcdef1234567890');

  const blockerReceipt = buildContractReceipt({
    candidateSha: 'abcdef1234567890',
    plan,
    componentResults: [{
      candidateSha: 'abcdef1234567890',
      foundation: 'native-react',
      status: 'failed',
      exitCode: 1,
      warnings: [],
      errors: ['standard component contract failed'],
    }],
  });
  assert.equal(blockerReceipt.status, 'failed');

  const staleReceipt = buildContractReceipt({
    candidateSha: 'abcdef1234567890',
    plan,
    componentResults: [{
      candidateSha: 'stale00000000000',
      foundation: 'native-react',
      status: 'passed',
      exitCode: 0,
      warnings: [],
      warningFiles: [],
      errors: [],
    }],
  });
  assert.equal(staleReceipt.status, 'failed');
  assert.match(staleReceipt.errors[0], /candidate SHA mismatch/u);
});

test('AC-001/009 foundation receipt adapts into the unified quality gate component', () => {
  const report = buildQualityGateComponentReport({
    candidateSha: 'abcdef1234567890',
    status: 'passed',
    exitCode: 0,
    warnings: ['review deferred browser evidence'],
    warningFiles: ['tmp/test-governance/foundation.warnings.log'],
    errors: [],
    deferredEvidence: ['native-react: nightly browser matrix'],
  });

  assert.equal(report.scope, 'foundation-contracts');
  assert.equal(report.status, 'passed');
  assert.equal(report.exitCode, 0);
  assert.equal(report.commit, 'abcdef1234567890');
  assert.deepEqual(report.warningFiles, ['tmp/test-governance/foundation.warnings.log']);
  assert.deepEqual(report.foundationContractWarnings, ['review deferred browser evidence']);
  assert.deepEqual(report.deferredEvidence, ['native-react: nightly browser matrix']);

  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'foundation-quality-component-'));
  const artifacts = writeQualityGateComponentArtifacts(repoRoot, {
    candidateSha: 'abcdef1234567890',
    status: 'passed',
    exitCode: 0,
    warnings: [],
    warningFiles: [],
    errors: [],
    uncovered: [],
    deferredEvidence: [],
  });
  assert.equal(fs.existsSync(artifacts.reportPath), true);
  assert.equal(fs.existsSync(artifacts.logPath), true);
  assert.equal(JSON.parse(fs.readFileSync(artifacts.reportPath, 'utf8')).scope, 'foundation-contracts');
});


test('shared Kernel and wire contracts select the three backend foundations', () => {
  for (const file of [
    'api/crates/interface-runtime/src/kernel.rs',
    'api/crates/interface-runtime/src/_tests/kernel_tests.rs',
    'api/crates/interface-runtime/Cargo.toml',
    'api/crates/extension-contracts/src/managed_hooks.rs',
    'api/crates/extension-contracts/src/_tests/wire_tests.rs',
    'api/crates/extension-contracts/Cargo.toml',
    'api/crates/runtime-core/src/runtime_backend.rs',
    'api/crates/runtime-core/src/runtime_backend/network_egress.rs',
    'api/crates/runtime-core/src/_tests/runtime_backend_tests.rs',
  ]) {
    const plan = buildFoundationPlan({ changedFiles: [file] });
    assert.deepEqual(plan.selectedFoundations, ['ai-gateway', 'mcp-gateway', 'application-backend'], file);
    for (const id of plan.selectedFoundations) assert.deepEqual(plan.packs[id].triggerReasons, [`changed: ${file}`]);
  }
});

test('shared contract routing does not select documentation or unrelated modules', () => {
  for (const file of [
    'api/crates/interface-runtime/AGENTS.md',
    'api/crates/extension-contracts/README.md',
    'api/crates/runtime-core/src/capability_slots.rs',
    'api/crates/runtime-core/src/runtime_backend_notes.rs',
    'api/crates/runtime-core/src/_tests/other_tests.rs',
    'api/crates/interface-runtime/src/i18n/en_US.json',
    'web/app/src/features/frontstage/styles.css',
  ]) assert.deepEqual(buildFoundationPlan({ changedFiles: [file] }).selectedFoundations, [], file);
});
