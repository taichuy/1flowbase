const path = require('node:path');

const { backendThresholds } = require('../testing/coverage-thresholds.js');
const {
  BACKEND_CI_TEST_SHARDS,
  BACKEND_SHARDS,
} = require('../verify/index.js');

const REPO_BACKEND_SHARD_TARGETS = ['clippy', 'test', 'check'];
const REPO_BACKEND_SHARDS_BY_TARGET = {
  clippy: BACKEND_SHARDS,
  test: BACKEND_CI_TEST_SHARDS,
  check: BACKEND_SHARDS,
};
const REPO_BACKEND_COMPONENT_SCOPES = [
  'repo-backend-static',
  'repo-backend-fmt',
  'repo-backend-image-llm-vision',
  'repo-backend-official-i18n-seed',
  ...REPO_BACKEND_SHARD_TARGETS.flatMap((target) =>
    REPO_BACKEND_SHARDS_BY_TARGET[target].map((shard) => `repo-backend-${target}-${shard.key}`)
  ),
];
const COVERAGE_BACKEND_COMPONENT_SCOPES = backendThresholds.map((entry) => `coverage-backend-${entry.key}`);
const COVERAGE_API_SERVER_SHARDED_SCOPES = Array.from(
  { length: 4 },
  (_, index) => `coverage-backend-api-server-sharded-${index + 1}-of-4`
);
const DEFAULT_AGGREGATE_SCOPES = [
  'repo-tooling',
  'repo-frontend',
  ...REPO_BACKEND_COMPONENT_SCOPES,
  'backend-consistency',
  'coverage-frontend',
  ...COVERAGE_BACKEND_COMPONENT_SCOPES,
];
const VALID_SCOPES = new Set([
  'ci',
  'repo',
  'repo-tooling',
  'repo-frontend',
  'repo-frontend-pr',
  'repo-frontend-react-doctor',
  'repo-backend',
  'backend',
  'backend-consistency',
  'state-protocols',
  'coverage',
  'coverage-frontend',
  'coverage-backend',
  'container-images',
  'release-rollback',
  ...REPO_BACKEND_COMPONENT_SCOPES,
  ...COVERAGE_BACKEND_COMPONENT_SCOPES,
  ...COVERAGE_API_SERVER_SHARDED_SCOPES,
]);
const PACKED_CLI_ENTRIES = new Set(['container-image-security', 'verify-backend-consistency']);

function resolveCliEntry(repoRoot, entryName) {
  const cliDir = PACKED_CLI_ENTRIES.has(entryName)
    ? path.join('scripts', 'node', 'cli')
    : path.join('scripts', 'node');
  return path.join(repoRoot, cliDir, `${entryName}.js`);
}

function buildGateCommand({ repoRoot, scope }) {
  if (!VALID_SCOPES.has(scope)) {
    throw new Error(`Unknown quality gate scope: ${scope}`);
  }

  const command = process.execPath;

  if (scope === 'coverage') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'verify-coverage'), 'all'],
      cwd: repoRoot,
    };
  }

  if (scope === 'coverage-frontend') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'verify-coverage'), 'frontend'],
      cwd: repoRoot,
    };
  }

  if (scope === 'coverage-backend') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'verify-coverage'), 'backend'],
      cwd: repoRoot,
    };
  }

  if (scope.startsWith('coverage-backend-')) {
    const shardedMatch = scope.match(/^coverage-backend-api-server-sharded-(?<index>[1-4])-of-4$/u);
    if (shardedMatch) {
      return {
        command,
        args: [resolveCliEntry(repoRoot, 'coverage-shadow'), 'shard', 'api-server', shardedMatch.groups.index, '4'],
        cwd: repoRoot,
      };
    }
    return {
      command,
      args: [
        resolveCliEntry(repoRoot, 'verify-coverage'),
        'backend',
        scope.replace(/^coverage-backend-/u, ''),
      ],
      cwd: repoRoot,
    };
  }

  if (scope === 'repo-tooling') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'verify-repo'), 'tooling'],
      cwd: repoRoot,
    };
  }

  if (scope === 'repo-frontend') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'verify-repo'), 'frontend'],
      cwd: repoRoot,
    };
  }

  if (scope === 'repo-frontend-pr') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'verify-repo'), 'frontend-pr'],
      cwd: repoRoot,
    };
  }

  if (scope === 'repo-frontend-react-doctor') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'react-doctor-gate')],
      cwd: repoRoot,
    };
  }

  if (scope === 'repo-backend') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'verify-repo'), 'backend'],
      cwd: repoRoot,
    };
  }

  if (scope === 'repo-backend-static') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'verify-backend'), 'static'],
      cwd: repoRoot,
    };
  }

  if (scope === 'repo-backend-fmt') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'verify-backend'), 'fmt'],
      cwd: repoRoot,
    };
  }

  if (scope === 'repo-backend-image-llm-vision') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'verify-backend'), 'image-llm-vision'],
      cwd: repoRoot,
    };
  }

  if (scope === 'repo-backend-official-i18n-seed') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'verify-backend'), 'official-i18n-seed'],
      cwd: repoRoot,
    };
  }

  if (scope === 'container-images') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'container-image-security')],
      cwd: repoRoot,
    };
  }

  if (scope === 'release-rollback') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'release-rollback-gate')],
      cwd: repoRoot,
    };
  }

  if (scope === 'state-protocols') {
    return {
      command,
      args: [resolveCliEntry(repoRoot, 'verify-state-protocols'), '--skip-live-acp'],
      cwd: repoRoot,
    };
  }

  for (const target of REPO_BACKEND_SHARD_TARGETS) {
    for (const shard of REPO_BACKEND_SHARDS_BY_TARGET[target]) {
      if (scope === `repo-backend-${target}-${shard.key}`) {
        return {
          command,
          args: [resolveCliEntry(repoRoot, 'verify-backend'), target, shard.key],
          cwd: repoRoot,
        };
      }
    }
  }

  return {
    command,
    args: [resolveCliEntry(repoRoot, `verify-${scope}`)],
    cwd: repoRoot,
  };
}

module.exports = {
  buildGateCommand,
  COVERAGE_BACKEND_COMPONENT_SCOPES,
  DEFAULT_AGGREGATE_SCOPES,
  REPO_BACKEND_COMPONENT_SCOPES,
};
