const { BACKEND_CONSISTENCY_TARGETS, OFFICIAL_I18N_SEED_GATE_TARGETS } = require('../backend-targets.js');

function workspaceEvidence(missingLabel) {
  const suites = new Map();
  for (const target of [...BACKEND_CONSISTENCY_TARGETS, ...OFFICIAL_I18N_SEED_GATE_TARGETS]) {
    const binary = target.testTarget || target.packageName.replaceAll('-', '_');
    if (!suites.has(binary)) suites.set(binary, []);
    const name = target.filter ? `${target.filter}protected_case` : 'integration_case';
    suites.get(binary).push({ name, passed: target.label !== missingLabel });
  }
  return {
    stderr: [...suites.keys()].map((binary) => `     Running unittests src/lib.rs (target/debug/deps/${binary}-abcdef)\n`).join(''),
    stdout: [...suites.values()].map((tests) => {
      const passed = tests.filter((test) => test.passed).length;
      return `running ${tests.length} tests\n${tests.map((test) => `test ${test.name} ... ${test.passed ? 'ok' : 'ignored'}\n`).join('')}test result: ok. ${passed} passed; 0 failed; ${tests.length - passed} ignored;\n`;
    }).join('\n'),
  };
}

module.exports = { workspaceEvidence };
