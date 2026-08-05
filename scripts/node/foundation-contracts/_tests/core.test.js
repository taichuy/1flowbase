const test = require('node:test');
const assert = require('node:assert/strict');

const {
  buildContractReceipt,
  buildFoundationPlan,
  validatePackInventory,
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

test('AC-002 keeps mcp.result outside the core operations and only adds continuation evidence on risk', () => {
  assert.doesNotThrow(() => validatePackInventory());

  assert.throws(
    () => validatePackInventory({ mcpCoreOperations: ['mcp.list', 'mcp.get', 'mcp.result', 'mcp.call'] }),
    /mcp\.list -> mcp\.get -> mcp\.call/u,
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
