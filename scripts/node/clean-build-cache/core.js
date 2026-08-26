const fs = require('node:fs');
const path = require('node:path');

const SCOPES = new Set(['all', 'backend', 'frontend']);

const BACKEND_TARGETS = [
  path.join('api', 'target'),
];

const FRONTEND_TARGETS = [
  path.join('web', '.turbo'),
  path.join('web', 'app', '.turbo'),
  path.join('web', 'app', 'dist'),
];

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(`用法：node scripts/node/clean-build-cache/cli.js [all|backend|frontend] [选项]

默认范围：all，真实删除后端和前端构建缓存。
真实清理前会先停止 api-server；dry-run 不停止进程、不删除文件。

范围：
  all            清理 api/target + 前端构建缓存
  backend        仅清理 api/target
  frontend       仅清理前端构建缓存

选项：
  --backend-only   等价于 backend
  --frontend-only  等价于 frontend
  --dry-run        只打印将要清理的路径
  -h, --help       查看帮助
`);
}

function setScope(options, nextScope) {
  if (options.scopeSpecified && options.scope !== nextScope) {
    throw new Error('不能同时指定多个清理范围');
  }

  options.scope = nextScope;
  options.scopeSpecified = true;
}

function parseCliArgs(argv) {
  const options = {
    dryRun: false,
    help: false,
    scope: 'all',
    scopeSpecified: false,
  };

  for (const arg of argv) {
    if (arg === '-h' || arg === '--help') {
      options.help = true;
      continue;
    }

    if (arg === '--dry-run') {
      options.dryRun = true;
      continue;
    }

    if (arg === '--backend-only') {
      setScope(options, 'backend');
      continue;
    }

    if (arg === '--frontend-only') {
      setScope(options, 'frontend');
      continue;
    }

    if (arg.startsWith('-')) {
      throw new Error(`未知选项：${arg}`);
    }

    if (!SCOPES.has(arg)) {
      throw new Error(`未知清理范围：${arg}`);
    }

    setScope(options, arg);
  }

  return {
    dryRun: options.dryRun,
    help: options.help,
    scope: options.scope,
  };
}

function assertInsideRepo(repoRoot, absolutePath) {
  const relativePath = path.relative(repoRoot, absolutePath);
  if (relativePath === '' || relativePath.startsWith('..') || path.isAbsolute(relativePath)) {
    throw new Error(`拒绝处理仓库外路径：${absolutePath}`);
  }
}

function resolveTarget(repoRoot, relativePath) {
  const normalizedRelativePath = path.normalize(relativePath);
  const absolutePath = path.resolve(repoRoot, normalizedRelativePath);
  assertInsideRepo(repoRoot, absolutePath);

  return {
    absolutePath,
    relativePath: normalizedRelativePath,
  };
}

function getScopeTargets(scope) {
  switch (scope) {
    case 'backend':
      return BACKEND_TARGETS;
    case 'frontend':
      return FRONTEND_TARGETS;
    default:
      return [...BACKEND_TARGETS, ...FRONTEND_TARGETS];
  }
}

function buildCleanupPlan({ repoRoot = getRepoRoot(), scope }) {
  if (!SCOPES.has(scope)) {
    throw new Error(`未知清理范围：${scope}`);
  }

  return {
    scope,
    targets: getScopeTargets(scope).map((target) => resolveTarget(repoRoot, target)),
  };
}

function collectExistingTargets(targets) {
  return targets.filter((target) => fs.existsSync(target.absolutePath));
}

function logTargetList({ action, targets, writeStdout }) {
  writeStdout(`[1flowbase-clean-build-cache] ${action}:\n`);
  for (const target of targets) {
    writeStdout(`- ${target.relativePath}\n`);
  }
}

async function stopBackendServices({ repoRoot = getRepoRoot(), writeStdout = (text) => process.stdout.write(text) } = {}) {
  const {
    ensureRuntimeDirs,
    getRuntimePaths,
    getServiceDefinitions,
  } = require('../dev-up/services.js');
  const { stopService } = require('../dev-up/process.js');

  ensureRuntimeDirs(getRuntimePaths(repoRoot));

  const serviceDefinitions = getServiceDefinitions(repoRoot);
  const logImpl = (message) => {
    writeStdout(`[1flowbase-clean-build-cache] ${message}\n`);
  };

  for (const key of ['api-server']) {
    await stopService(serviceDefinitions[key], { logImpl });
  }
}

async function runBuildCacheCleanup({
  repoRoot = getRepoRoot(),
  options = parseCliArgs([]),
  removePathImpl = (targetPath) => fs.rmSync(targetPath, { recursive: true, force: true }),
  stopBackendServicesImpl = stopBackendServices,
  writeStdout = (text) => process.stdout.write(text),
} = {}) {
  if (options.help) {
    usage(writeStdout);
    return 0;
  }

  const plan = buildCleanupPlan({ repoRoot, scope: options.scope });
  const existingTargets = collectExistingTargets(plan.targets);

  if (existingTargets.length === 0) {
    writeStdout(`[1flowbase-clean-build-cache] ${options.scope} 没有发现可清理的构建缓存。\n`);
    return 0;
  }

  if (options.dryRun) {
    writeStdout('[1flowbase-clean-build-cache] dry-run：不会停止进程，也不会删除文件。\n');
    logTargetList({
      action: `would remove ${options.scope}`,
      targets: existingTargets,
      writeStdout,
    });
    return 0;
  }

  writeStdout('[1flowbase-clean-build-cache] 正在停止 api-server...\n');
  await stopBackendServicesImpl({ repoRoot, writeStdout });

  for (const target of existingTargets) {
    removePathImpl(target.absolutePath);
  }

  logTargetList({
    action: `removed ${options.scope}`,
    targets: existingTargets,
    writeStdout,
  });
  return 0;
}

module.exports = {
  BACKEND_TARGETS,
  FRONTEND_TARGETS,
  buildCleanupPlan,
  getRepoRoot,
  parseCliArgs,
  runBuildCacheCleanup,
  stopBackendServices,
  usage,
};
