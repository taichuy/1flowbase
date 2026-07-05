const test = require('node:test');
const assert = require('node:assert/strict');

const { main } = require('../index.js');

test('tooling index dispatches runtime-gate subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['runtime-gate', 'snapshot', '/settings'], {
    runRuntimeGateImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['snapshot', '/settings']);
});

test('tooling index dispatches check-style-boundary subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['check-style-boundary', 'all-pages'], {
    runCheckStyleBoundaryImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['all-pages']);
});

test('tooling index dispatches api-debug subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['api-debug', 'GET', '/api/console/me'], {
    runApiDebugImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['GET', '/api/console/me']);
});

test('tooling index dispatches check-rust-backend subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['check-rust-backend'], {
    runCheckRustBackendImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, []);
});

test('tooling index dispatches hotspot-review subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['hotspot-review', '--since', '1 day ago'], {
    runHotspotReviewImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['--since', '1 day ago']);
});

test('tooling index dispatches growth-table-report subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['growth-table-report', '--max-evidence', '2'], {
    runGrowthTableReportImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['--max-evidence', '2']);
});

test('tooling index dispatches raw-jsonb-report subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['raw-jsonb-report', '--max-evidence', '2'], {
    runRawJsonbReportImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['--max-evidence', '2']);
});

test('tooling index dispatches log-query-contract-report subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['log-query-contract-report'], {
    runLogQueryContractReportImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, []);
});

test('tooling index dispatches capacity-report subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['capacity-report', '--inspection-input', 'tmp/capacity.json'], {
    runCapacityReportImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['--inspection-input', 'tmp/capacity.json']);
});

test('tooling index dispatches repo-hygiene subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['repo-hygiene', '--max-findings', '10'], {
    runRepoHygieneImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['--max-findings', '10']);
});

test('tooling index dispatches console-route-registry-hygiene subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['console-route-registry-hygiene', '--max-findings', '10'], {
    runConsoleRouteRegistryHygieneImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['--max-findings', '10']);
});

test('tooling index dispatches frontstage-governance-hygiene subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['frontstage-governance-hygiene', '--max-findings', '10'], {
    runFrontstageGovernanceHygieneImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['--max-findings', '10']);
});

test('tooling index dispatches i18n-hygiene subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['i18n-hygiene', '--max-findings', '10'], {
    runI18nHygieneImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['--max-findings', '10']);
});

test('tooling index dispatches schema-hygiene subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['schema-hygiene', '--max-findings', '10'], {
    runSchemaHygieneImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['--max-findings', '10']);
});

test('tooling index dispatches security-risk subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['security-risk', 'origin/latest'], {
    runSecurityRiskImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['origin/latest']);
});

test('tooling index dispatches vite-lazy-deps-gate subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['vite-lazy-deps-gate', '--smoke'], {
    runViteLazyDepsGateImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['--smoke']);
});

test('tooling index dispatches gate-router subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['gate-router', '--staged'], {
    runGateRouterImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['--staged']);
});

test('tooling index passes subcommand help through to the subcommand', async () => {
  let capturedArgv = null;

  const status = await main(['claude-skill-sync', '--help'], {
    runClaudeSkillSyncImpl(argv) {
      capturedArgv = argv;
      return 0;
    },
  });

  assert.equal(status, 0);
  assert.deepEqual(capturedArgv, ['--help']);
});

test('tooling index rejects unknown subcommands', async () => {
  await assert.rejects(
    () => main(['unknown']),
    /Unknown tooling command: unknown/u
  );
});
