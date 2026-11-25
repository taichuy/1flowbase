const { spawnSync } = require('node:child_process');
const { API_SOURCE_OWNERS } = require('../testing/contracts/api-source-owners.js');

const { getRepoRoot } = require('../testing/warning-capture.js');
const { buildFoundationPlan } = require('../foundation-contracts/core.js');

const DEFAULT_BASE_REF = 'origin/main';
const CHANGED_FILES_ENV = 'GATE_ROUTER_CHANGED_FILES';

const IGNORED_PATH_PATTERNS = [
  /^api\/target(?:\/|$)/u,
  /^web\/node_modules(?:\/|$)/u,
  /^web\/app\/node_modules(?:\/|$)/u,
  /^web\/app\/dist(?:\/|$)/u,
  /^web\/app\/tmp(?:\/|$)/u,
  /^tmp(?:\/|$)/u,
  /(?:^|\/)\.turbo(?:\/|$)/u,
];

// Keep advisory routing aligned with the exact package owners used by CI.
const { BACKEND_SHARDS } = require('../verify/backend-targets.js');
const CHECK_SCOPE_BY_PACKAGE = new Map(BACKEND_SHARDS.flatMap(({ key, packages }) =>
  packages.map((packageName) => [packageName, `repo-backend-check-${key}`])
));

const BACKEND_CHECK_SCOPES = ['core-libs', 'runtime-storage', 'apps']
  .map((shard) => `repo-backend-check-${shard}`);
const BACKEND_BASE_SCOPES = ['repo-backend-static', 'repo-backend-fmt'];
const BACKEND_SPECIAL_SCOPES = ['repo-backend-image-llm-vision', 'repo-backend-official-i18n-seed'];
const ALL_BACKEND_SCOPES = [...BACKEND_BASE_SCOPES, ...BACKEND_SPECIAL_SCOPES, ...BACKEND_CHECK_SCOPES];

const ROUTE_DEFINITIONS = [
  {
    scope: 'repo-tooling',
    command: 'node scripts/node/verify-repo.js tooling',
    reason: 'repository tooling, workflow, governance, i18n, or contract files changed',
  },
  {
    scope: 'repo-frontend-pr',
    command: 'node scripts/node/verify-repo.js frontend-pr',
    reason: 'frontend files changed',
  },
  {
    scope: 'repo-backend-static',
    command: 'node scripts/node/verify-backend.js static',
    reason: 'backend Rust files changed',
  },
  {
    scope: 'repo-backend-fmt',
    command: 'node scripts/node/verify-backend.js fmt',
    reason: 'backend Rust files changed',
  },
  {
    scope: 'repo-backend-check-core-libs',
    command: 'node scripts/node/verify-backend.js check core-libs',
    reason: 'backend core library crates changed',
  },
  {
    scope: 'repo-backend-check-runtime-storage',
    command: 'node scripts/node/verify-backend.js check runtime-storage',
    reason: 'backend runtime or storage crates changed',
  },
  {
    scope: 'repo-backend-check-apps',
    command: 'node scripts/node/verify-backend.js check apps',
    reason: 'backend application crates changed',
  },
  {
    scope: 'backend-consistency',
    command: 'node scripts/node/verify.js backend-consistency',
    reason: 'state, runtime, route, repository, or storage consistency paths changed',
  },
  {
    scope: 'state-protocols',
    command: 'node scripts/node/verify-state-protocols.js',
    reason: 'ACP, Anthropic-compatible SSE, or public agent protocol state projection changed',
  },
  {
    scope: 'container-images',
    command: 'GitHub Actions quality-gate scope=container-images',
    reason: 'container or deployment files changed',
  },
  ...[
    ['ai-gateway', 'AI Gateway protocol or provider-facing contract paths changed'],
    ['mcp-gateway', 'MCP discovery, invocation, ACL, mapping, or continuation paths changed'],
    ['application-backend', 'Application Backend data model, runtime API, scope, or metadata paths changed'],
    ['native-react', 'Native React Component, compiler/runtime ABI, dependency, capability, or artifact paths changed'],
  ].map(([foundation, reason]) => ({
    scope: `foundation-contract-${foundation}-fast`,
    command: `node scripts/node/tooling.js foundation-contracts run-fast --foundation ${foundation} --candidate-sha HEAD`,
    reason,
  })),
];

function normalizePath(filePath) {
  return filePath.replace(/\\/gu, '/').trim();
}

function uniqueSorted(values) {
  return [...new Set(values)].sort();
}

function isIgnoredPath(filePath) {
  return IGNORED_PATH_PATTERNS.some((pattern) => pattern.test(filePath));
}

function splitChangedFiles(value) {
  return uniqueSorted(
    value
      .split(/\r?\n/u)
      .map(normalizePath)
      .filter(Boolean)
      .filter((filePath) => !isIgnoredPath(filePath))
  );
}

function parseCliArgs(argv = []) {
  if (argv.includes('-h') || argv.includes('--help')) {
    return {
      help: true,
      mode: 'branch',
      baseRef: DEFAULT_BASE_REF,
    };
  }

  let mode = 'branch';
  let baseRef;
  let prPlan = false;

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '--pr-plan-json') {
      prPlan = true;
      continue;
    }

    if (arg === '--staged') {
      mode = 'staged';
      continue;
    }

    if (arg === '--branch') {
      mode = 'branch';
      continue;
    }

    if (arg === '--base-ref') {
      const next = argv[index + 1];
      if (!next) {
        throw new Error('--base-ref requires a value');
      }
      baseRef = next;
      index += 1;
      continue;
    }

    throw new Error(`Unknown gate-router option: ${arg}`);
  }

  return {
    help: false,
    mode,
    baseRef: baseRef ?? (prPlan ? null : DEFAULT_BASE_REF),
    ...(prPlan && { prPlan }),
  };
}

function buildGitDiffArgs({ mode, baseRef }) {
  const commonArgs = ['diff', '--name-only', '--no-renames', '--diff-filter=ACDMRTUXB'];

  if (mode === 'staged') {
    return [...commonArgs, '--cached'];
  }

  // CI compares the exact candidate and base trees, including force-pushed changes.
  if (mode === 'candidate') return [...commonArgs, '-z', baseRef, 'HEAD', '--'];
  return [...commonArgs, `${baseRef}...HEAD`];
}

function readChangedFiles({
  repoRoot,
  mode,
  baseRef,
  env = process.env,
  spawnSyncImpl = spawnSync,
}) {
  if (mode !== 'candidate' && env[CHANGED_FILES_ENV] !== undefined) {
    return splitChangedFiles(env[CHANGED_FILES_ENV]);
  }

  const result = spawnSyncImpl('git', buildGitDiffArgs({ mode, baseRef }), {
    cwd: repoRoot,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(result.stderr || result.stdout || 'git diff failed');
  }

  return mode === 'candidate'
    ? uniqueSorted(result.stdout.split('\0').filter(Boolean))
    : splitChangedFiles(result.stdout);
}

function addRoute(routes, scope) {
  const route = ROUTE_DEFINITIONS.find((candidate) => candidate.scope === scope);
  if (!route || routes.has(scope)) {
    return;
  }

  routes.set(scope, route);
}

function isToolingFile(filePath) {
  return /^scripts\//u.test(filePath)
    || /^\.github\//u.test(filePath)
    || /^\.githooks\//u.test(filePath)
    || /^\.agents\//u.test(filePath)
    || /^\.memory\/feedback-memory\//u.test(filePath)
    || /(?:^|\/)AGENTS\.md$/u.test(filePath)
    || /(?:^|\/)_tests\//u.test(filePath)
    || /(?:^|\/)(?:i18n|locales)(?:\/|$)/u.test(filePath)
    || /^web\/.*\/i18n\//u.test(filePath)
    || /^web\/.*\/locales\//u.test(filePath);
}

function isFrontendFile(filePath) {
  return /^web\//u.test(filePath);
}

function isBackendFile(filePath) {
  return /^api\/(?:apps|crates|plugins|Cargo\.(?:toml|lock))/u.test(filePath);
}

function readCrateFromPath(filePath) {
  const apiOwner = API_SOURCE_OWNERS.find((owner) => filePath.startsWith(`${owner.directory}/`));
  if (apiOwner) return apiOwner.packageName;
  const crateMatch = /^api\/crates\/([^/]+)\//u.exec(filePath);
  if (crateMatch) {
    return crateMatch[1];
  }

  const appMatch = /^api\/apps\/([^/]+)\//u.exec(filePath);
  if (appMatch) {
    return appMatch[1];
  }

  return null;
}

function addBackendCheckRoutes(routes, filePath) {
  if (/^api\/Cargo\.(?:toml|lock)$/u.test(filePath)) {
    addRoute(routes, 'repo-backend-check-core-libs');
    addRoute(routes, 'repo-backend-check-runtime-storage');
    addRoute(routes, 'repo-backend-check-apps');
    return;
  }

  const crate = readCrateFromPath(filePath);

  // Storage crate directories intentionally group durable, ephemeral and object
  // packages under one owner; package names otherwise match crate directories.
  const checkScope = crate === 'storage'
    ? 'repo-backend-check-runtime-storage'
    : CHECK_SCOPE_BY_PACKAGE.get(crate);
  if (checkScope) {
    addRoute(routes, checkScope);
    return;
  }

  addRoute(routes, 'repo-backend-check-core-libs');
  addRoute(routes, 'repo-backend-check-runtime-storage');
  addRoute(routes, 'repo-backend-check-apps');
}

function isBackendConsistencyFile(filePath) {
  return isBackendFile(filePath)
    && (/^api\/crates\/(?:control-plane|orchestration-runtime|runtime-core|storage(?:\/|$))/u.test(filePath)
    || /^api\/(?:apps\/api-server|crates\/api\/[^/]+)\/src\/(?:routes|contracts\/routes|slices\/routes)\//u.test(filePath)
    || /(?:^|\/)migrations?(?:\/|$)/iu.test(filePath)
    || /(?:state|transition|repository|workspace|runtime|model_definition|orchestration|permission|acl|access-control)/iu.test(filePath));
}

function isStateProtocolFile(filePath) {
  return /^scripts\/node\/(?:acp-claude-smoke\/|cli\/acp-claude-smoke\.js$|verify\/state-protocols\.js$|verify-state-protocols(?:\.js|\/))/u.test(filePath)
    || /^api\/(?:apps\/api-server|crates\/api\/public-gateway)\/src\/routes\/application_public_api\/(?:anthropic\.rs$|compat_sse(?:\/|\.rs$))/u.test(filePath);
}

function isContainerFile(filePath) {
  return /^docker\//u.test(filePath)
    || /^\.github\/workflows\/container-images\.yml$/u.test(filePath)
    || /(?:^|\/)(?:Dockerfile|docker-compose[^/]*\.ya?ml|nginx\.conf)$/u.test(filePath);
}

function routeChangedFiles(changedFiles) {
  const routes = new Map();
  const normalizedFiles = splitChangedFiles(changedFiles.join('\n'));

  for (const filePath of normalizedFiles) {
    if (isToolingFile(filePath)) {
      addRoute(routes, 'repo-tooling');
    }

    if (isFrontendFile(filePath)) {
      addRoute(routes, 'repo-frontend-pr');
    }

    if (isBackendFile(filePath)) {
      addRoute(routes, 'repo-backend-static');
      addRoute(routes, 'repo-backend-fmt');
      addBackendCheckRoutes(routes, filePath);
    }

    if (isBackendConsistencyFile(filePath)) {
      addRoute(routes, 'backend-consistency');
    }

    if (isStateProtocolFile(filePath)) {
      addRoute(routes, 'state-protocols');
    }

    if (isContainerFile(filePath)) {
      addRoute(routes, 'container-images');
    }
  }

  const foundationPlan = buildFoundationPlan({ changedFiles: normalizedFiles });
  for (const foundation of foundationPlan.selectedFoundations) {
    addRoute(routes, `foundation-contract-${foundation}-fast`);
  }

  return ROUTE_DEFINITIONS.filter((route) => routes.has(route.scope));
}

function buildPrVerifyPlan({ changedFiles = [] } = {}) {
  const files = changedFiles.filter((file) => !isIgnoredPath(file));
  const global = files.length === 0 || files.some((file) =>
    !/^(?:api|web|docs)\//u.test(file) || /(?:[./-]lock(?:b|file)?(?:\.ya?ml|\.json)?|shrinkwrap\.(?:ya?ml|json))$/iu.test(file)
    || /^api\/(?:Cargo\.toml|\.cargo\/|rust-toolchain)/u.test(file));
  const webSeed = files.some((file) => isFrontendFile(file)
    && /(?:i18n|locales?)[-_.\/].*seed|seed[-_.\/].*(?:i18n|locales?)/iu.test(file));
  const tooling = global || webSeed || files.some(isToolingFile);
  const frontend = global || files.some(isFrontendFile);
  const backendFiles = files.filter((file) => file.startsWith('api/'));
  let backendScopes = [];
  if (global || webSeed) {
    backendScopes = [...ALL_BACKEND_SCOPES];
  } else if (backendFiles.length) {
    const routes = new Map();
    backendFiles.forEach((file) => addBackendCheckRoutes(routes, file));
    const firstImpacted = BACKEND_CHECK_SCOPES.findIndex((scope) => routes.has(scope));
    const specialistPaths = backendFiles.some((file) => /i18n|locale|seed|image|llm|vision|model/iu.test(file));
    backendScopes = [
      ...BACKEND_BASE_SCOPES,
      ...(specialistPaths ? BACKEND_SPECIAL_SCOPES : []),
      ...BACKEND_CHECK_SCOPES.slice(Math.max(0, firstImpacted)),
    ];
  }
  const expectedScopes = [
    ...(tooling ? ['repo-tooling'] : []),
    ...(frontend ? ['repo-frontend-pr'] : []),
    ...backendScopes,
    // The reusable workflow always produces a SHA-bound receipt, even for no impact.
    'foundation-contracts',
  ];
  return { tooling, frontend, backendScopes, expectedScopes, allFoundations: global,
    selectedFoundations: buildFoundationPlan({ changedFiles: files, foundation: global ? 'all' : 'auto' }).selectedFoundations };
}

function readPrVerifyPlan({ baseRef, ...options }) {
  if (!/^(?!0+$)[0-9a-f]{40}$/u.test(baseRef ?? '')) return buildPrVerifyPlan();
  try {
    return buildPrVerifyPlan({ changedFiles: readChangedFiles({ ...options, baseRef, mode: 'candidate' }) });
  } catch {
    return buildPrVerifyPlan();
  }
}

function buildAdvisoryMessage({ mode, changedFiles, routes }) {
  if (!routes.length) {
    return '';
  }

  const modeLabel = mode === 'staged' ? 'staged changes' : 'branch changes';
  const lines = [
    '[1flowbase-gate-router] Advisory only: changed files may need quality gates before push.',
    `Mode: ${modeLabel}`,
    `Changed files inspected: ${changedFiles.length}`,
    'Recommended gates:',
    ...routes.map((route) => `- ${route.scope}: ${route.command} (${route.reason})`),
    'This hook does not block the commit. Run the gates when the change is ready.',
    '',
  ];

  return lines.join('\n');
}

function buildReadFailureMessage(error) {
  const detail = String(error?.message || error || 'unknown error').replace(/\s+/gu, ' ').trim();

  return [
    '[1flowbase-gate-router] Advisory only: unable to inspect git changes.',
    `Reason: ${detail}`,
    'This hook does not block the commit.',
    '',
  ].join('\n');
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/tooling.js gate-router [--staged|--branch] [--base-ref <ref>] [--pr-plan-json]\n'
      + 'Prints non-blocking quality gate suggestions for changed files.\n'
  );
}

async function main(argv = [], deps = {}) {
  const options = parseCliArgs(argv);

  if (options.help) {
    usage(deps.writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const env = deps.env || process.env;
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));
  if (options.prPlan) {
    const plan = readPrVerifyPlan({ repoRoot, baseRef: options.baseRef, env, spawnSyncImpl: deps.spawnSyncImpl });
    writeStdout(`${JSON.stringify(plan)}\n`);
    return 0;
  }
  let changedFiles = [];

  try {
    changedFiles = readChangedFiles({
      repoRoot,
      mode: options.mode,
      baseRef: options.baseRef,
      env,
      spawnSyncImpl: deps.spawnSyncImpl,
    });
  } catch (error) {
    writeStdout(buildReadFailureMessage(error));
    return 0;
  }

  const routes = routeChangedFiles(changedFiles);
  const message = buildAdvisoryMessage({
    mode: options.mode,
    changedFiles,
    routes,
  });

  if (message) {
    writeStdout(message);
  }

  return 0;
}

module.exports = {
  buildAdvisoryMessage,
  buildGitDiffArgs,
  buildPrVerifyPlan,
  readPrVerifyPlan,
  main,
  parseCliArgs,
  readChangedFiles,
  routeChangedFiles,
  splitChangedFiles,
};
