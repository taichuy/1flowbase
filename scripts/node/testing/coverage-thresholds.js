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
    thresholds: {
      lines: 65,
      functions: 65,
      statements: 65,
      branches: 50,
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
  // Ratchet from the 2026-08-09 full beta artifact after Rust module/test
  // reassembly changed LLVM's instrumented-line denominator. Raise only when
  // a newer full artifact proves the recovered baseline.
  { key: 'control-plane', packageName: 'control-plane', line: 69 },
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
