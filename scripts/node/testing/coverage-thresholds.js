const COVERAGE_ROOT = 'tmp/test-governance/coverage';

const frontendThresholds = [
  {
    key: 'agent-flow',
    prefix: 'src/features/agent-flow/',
    thresholds: {
      lines: 70,
      functions: 70,
      statements: 70,
      branches: 55,
    },
  },
  {
    key: 'settings',
    prefix: 'src/features/settings/',
    // Ratchet from the 2026-08-09 full stable-pack artifact after long-running
    // page regression tests moved to their dedicated non-coverage lane.
    thresholds: {
      lines: 56,
      functions: 54,
      statements: 55,
      branches: 46,
    },
  },
  {
    key: 'page-runtime',
    prefix: 'packages/page-runtime/',
    thresholds: {
      lines: 60,
      functions: 60,
      statements: 60,
      branches: 45,
    },
  },
];

const backendThresholds = [
  // Ratchet from the 2026-08-10 full beta artifact after the selected
  // integration tests reported 67.29% across 113,108 instrumented lines.
  // Raise only when a newer full artifact proves the recovered baseline.
  { key: 'control-plane', packageName: 'control-plane', line: 67 },
  { key: 'orchestration-runtime', packageName: 'orchestration-runtime', line: 60 },
  { key: 'plugin-runner', packageName: 'plugin-runner', line: 55 },
  { key: 'storage-postgres', packageName: 'storage-postgres', line: 65 },
  { key: 'api-server', packageName: 'api-server', line: 60 },
];

module.exports = {
  COVERAGE_ROOT,
  frontendThresholds,
  backendThresholds,
};
