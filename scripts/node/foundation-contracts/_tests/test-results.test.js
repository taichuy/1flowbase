const test = require('node:test');
const assert = require('node:assert/strict');
const { parseExecutedTestCounts } = require('../test-results.js');
const { buildFoundationPlan, buildContractReceipt } = require('../core.js');

for (const [runner, output, expected] of [
  ['cargo', 'test result: ok. 3 passed; 0 failed; 1 ignored;', { passedCount: 3, failedCount: 0 }],
  ['vitest', '\u001b[2m Tests \u001b[22m \u001b[32m172 passed\u001b[39m (172)\n', { passedCount: 172, failedCount: 0 }],
  ['vitest', 'Tests 1 failed | 2 passed | 3 skipped (6)\n', { passedCount: 2, failedCount: 1 }],
  ['vitest', 'Test Files 2 passed (2)\nTests 2 skipped (2)\n', { passedCount: 0, failedCount: 0 }],
  ['vitest', 'No test files found, exiting with code 0\n', { passedCount: null, failedCount: null }],
  ['node', '# pass 5\n# fail 0\n# skipped 3\n', { passedCount: 5, failedCount: 0 }],
  ['node', 'ℹ pass 8\nℹ fail 1\n', { passedCount: 8, failedCount: 1 }],
  ['node', 'ℹ tests 0\nℹ pass 0\nℹ fail 0\n', { passedCount: 0, failedCount: 0 }],
]) {
  test(`${runner} recognizes actual execution summary ${JSON.stringify(output)}`, () => {
    assert.deepEqual(parseExecutedTestCounts(runner, output), expected);
  });
}

test('every foundation declares a test runner and native packs bypass passWithNoTests package scripts', () => {
  const plan = buildFoundationPlan({ foundation: 'all' });
  for (const pack of Object.values(plan.packs)) {
    for (const command of pack.fast) assert.match(command.testRunner, /^(?:cargo|node|vitest)$/u);
  }
  for (const command of plan.packs['native-react'].fast) {
    assert.ok(command.args.includes('exec'));
    assert.ok(command.args.includes('vitest'));
    assert.ok(command.args.includes('run'));
    assert.ok(!command.args.includes('test'));
  }
});

test('native and Node receipt success still requires actual passing tests', () => {
  for (const foundation of ['native-react', 'ai-gateway']) {
    const plan = buildFoundationPlan({ foundation });
    const pack = plan.packs[foundation].fast;
    const result = {
      foundation, candidateSha: 'candidate', status: 'passed', exitCode: 0,
      executedPack: pack.map((item) => item.id),
      commands: pack.map((item) => ({ id: item.id, exitCode: 0, logPath: `${item.id}.log`, passedCount: 2, failedCount: 0 })),
    };
    assert.equal(buildContractReceipt({ candidateSha: 'candidate', plan, componentResults: [result] }).status, 'passed');
    for (const counts of [{ passedCount: 0, failedCount: 0 }, { passedCount: null, failedCount: null }, { passedCount: 2, failedCount: 1 }]) {
      const invalid = structuredClone(result);
      Object.assign(invalid.commands[0], counts);
      assert.equal(buildContractReceipt({ candidateSha: 'candidate', plan, componentResults: [invalid] }).status, 'failed');
    }
  }
});


test('conditional MCP continuation rejects zero-test and missing-count receipts', () => {
  const plan = buildFoundationPlan({
    foundation: 'mcp-gateway',
    changedFiles: ['api/apps/api-server/src/routes/mcp_protocol/result_delivery.rs'],
  });
  const pack = plan.packs['mcp-gateway'].fast;
  assert.equal(pack.find((item) => item.id === 'mcp-result-continuation').testRunner, 'cargo');
  const result = {
    foundation: 'mcp-gateway', candidateSha: 'candidate', status: 'passed', exitCode: 0,
    executedPack: pack.map((item) => item.id),
    commands: pack.map((item) => ({ id: item.id, exitCode: 0, logPath: `${item.id}.log`, passedCount: 2, failedCount: 0 })),
  };
  assert.equal(buildContractReceipt({ candidateSha: 'candidate', plan, componentResults: [result] }).status, 'passed');
  for (const passedCount of [0, null]) {
    const invalid = structuredClone(result);
    invalid.commands.find((item) => item.id === 'mcp-result-continuation').passedCount = passedCount;
    assert.equal(buildContractReceipt({ candidateSha: 'candidate', plan, componentResults: [invalid] }).status, 'failed');
  }
});
