const fs = require('node:fs');
const path = require('node:path');

const {
  getRepoRoot,
  resolveOutputDir,
} = require('../testing/warning-capture.js');
const { loadPlaywright } = require('../page-debug/core.js');

const DEFAULT_MANIFEST_PATH = path.join(__dirname, 'manifest.json');
const DEFAULT_WEB_BASE_URL = 'http://127.0.0.1:3100';
const DEFAULT_TIMEOUT = 30_000;
const SOURCE_EXTENSIONS = ['.tsx', '.ts', '.jsx', '.js'];
const SIDE_EFFECT_IMPORT_PATTERN = /^\s*import\s+['"]([^'"]+)['"]\s*;?/gmu;
const IMPORT_FROM_PATTERN = /^\s*import\s+(?!type\b)(?!['"])(?:[\s\S]*?)\s+from\s*['"]([^'"]+)['"]\s*;?/gmu;
const EXPORT_FROM_PATTERN = /^\s*export\s+(?!type\b)(?:[\s\S]*?)\s+from\s+['"]([^'"]+)['"]\s*;?/gmu;
const DYNAMIC_IMPORT_PATTERN = /\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/gu;
const LAZY_IMPORT_PATTERN = /\b(?:React\.)?lazy\s*\(\s*\(\s*\)\s*=>\s*import\s*\(\s*['"]([^'"]+)['"]\s*\)/gu;

function normalizeRepoPath(filePath) {
  return filePath.replace(/\\/gu, '/');
}

function toRepoRelative(repoRoot, filePath) {
  return normalizeRepoPath(path.relative(repoRoot, filePath));
}

function readText(filePath) {
  return fs.readFileSync(filePath, 'utf8');
}

function stripComments(source) {
  return source
    .replace(/\/\*[\s\S]*?\*\//gu, '')
    .replace(/(^|[^:])\/\/.*$/gmu, '$1');
}

function lineNumberAt(source, index) {
  return source.slice(0, index).split(/\r?\n/u).length;
}

function uniqueSorted(values) {
  return [...new Set(values)].sort((left, right) => left.localeCompare(right));
}

function readWebPackageInfo(repoRoot) {
  const packagePath = path.join(repoRoot, 'web', 'app', 'package.json');
  const parsed = JSON.parse(readText(packagePath));
  return {
    dependencies: {
      ...(parsed.dependencies || {}),
      ...(parsed.devDependencies || {}),
      ...(parsed.peerDependencies || {}),
    },
  };
}

function packageRoot(specifier) {
  if (
    !specifier
    || specifier.startsWith('.')
    || specifier.startsWith('/')
    || specifier.startsWith('node:')
    || specifier.startsWith('\0')
    || specifier.startsWith('virtual:')
    || specifier.includes('${')
  ) {
    return null;
  }

  const parts = specifier.split('/');
  if (specifier.startsWith('@')) {
    return parts.length >= 2 ? `${parts[0]}/${parts[1]}` : specifier;
  }

  return parts[0];
}

function isWorkspacePackage(root, packageInfo) {
  if (root.startsWith('@1flowbase/')) {
    return true;
  }

  const version = packageInfo.dependencies[root];
  return typeof version === 'string' && version.startsWith('workspace:');
}

function isBareNpmImport(specifier, packageInfo) {
  const root = packageRoot(specifier);
  return Boolean(root) && !isWorkspacePackage(root, packageInfo);
}

function addDependency(dependencies, specifier, sourcePath, repoRoot, packageInfo) {
  if (!isBareNpmImport(specifier, packageInfo)) {
    return;
  }

  const root = packageRoot(specifier);
  if (!dependencies.has(root)) {
    dependencies.set(root, new Set());
  }
  dependencies.get(root).add(toRepoRelative(repoRoot, sourcePath));
}

function collectImportSpecifiers(source) {
  const stripped = stripComments(source);
  const imports = [];
  SIDE_EFFECT_IMPORT_PATTERN.lastIndex = 0;
  IMPORT_FROM_PATTERN.lastIndex = 0;
  EXPORT_FROM_PATTERN.lastIndex = 0;
  let match = SIDE_EFFECT_IMPORT_PATTERN.exec(stripped);

  while (match) {
    imports.push(match[1]);
    match = SIDE_EFFECT_IMPORT_PATTERN.exec(stripped);
  }

  match = IMPORT_FROM_PATTERN.exec(stripped);
  while (match) {
    imports.push(match[1]);
    match = IMPORT_FROM_PATTERN.exec(stripped);
  }

  match = EXPORT_FROM_PATTERN.exec(stripped);
  while (match) {
    imports.push(match[1]);
    match = EXPORT_FROM_PATTERN.exec(stripped);
  }

  return imports;
}

function collectDynamicImportSpecifiers(source) {
  const stripped = stripComments(source);
  const imports = [];
  DYNAMIC_IMPORT_PATTERN.lastIndex = 0;
  let match = DYNAMIC_IMPORT_PATTERN.exec(stripped);

  while (match) {
    imports.push(match[1]);
    match = DYNAMIC_IMPORT_PATTERN.exec(stripped);
  }

  return imports;
}

function resolveSourceFile(importerPath, specifier) {
  if (!specifier.startsWith('.')) {
    return null;
  }

  const basePath = path.resolve(path.dirname(importerPath), specifier);
  const candidates = [];
  const extension = path.extname(basePath);

  if (extension) {
    candidates.push(basePath);
  } else {
    for (const sourceExtension of SOURCE_EXTENSIONS) {
      candidates.push(`${basePath}${sourceExtension}`);
    }
    for (const sourceExtension of SOURCE_EXTENSIONS) {
      candidates.push(path.join(basePath, `index${sourceExtension}`));
    }
  }

  return candidates.find((candidate) => fs.existsSync(candidate) && fs.statSync(candidate).isFile()) || null;
}

function shouldVisitSourceFile(filePath) {
  const normalized = normalizeRepoPath(filePath);

  return SOURCE_EXTENSIONS.includes(path.extname(filePath))
    && !normalized.includes('/_tests/')
    && !normalized.includes('/src/test/')
    && !normalized.endsWith('.d.ts')
    && !normalized.includes('/src/style-boundary/');
}

function listSourceFiles(sourceRoot) {
  const collected = [];
  const entries = fs.readdirSync(sourceRoot, { withFileTypes: true });

  for (const entry of entries) {
    const absolutePath = path.join(sourceRoot, entry.name);
    if (entry.isDirectory()) {
      collected.push(...listSourceFiles(absolutePath));
      continue;
    }

    if (entry.isFile() && shouldVisitSourceFile(absolutePath)) {
      collected.push(absolutePath);
    }
  }

  return collected;
}

function collectModuleGraphDependencies({
  entryFiles,
  packageInfo,
  repoRoot,
  includeDynamicImports = false,
}) {
  const dependencies = new Map();
  const visited = new Set();
  const pending = [...entryFiles];

  while (pending.length > 0) {
    const currentPath = pending.pop();
    if (!currentPath || visited.has(currentPath) || !shouldVisitSourceFile(currentPath)) {
      continue;
    }

    visited.add(currentPath);
    const source = readText(currentPath);
    const specifiers = collectImportSpecifiers(source);
    if (includeDynamicImports) {
      specifiers.push(...collectDynamicImportSpecifiers(source));
    }

    for (const specifier of specifiers) {
      if (isBareNpmImport(specifier, packageInfo)) {
        addDependency(dependencies, specifier, currentPath, repoRoot, packageInfo);
        continue;
      }

      const resolved = resolveSourceFile(currentPath, specifier);
      if (resolved) {
        pending.push(resolved);
      }
    }
  }

  return {
    dependencies,
    visitedFiles: uniqueSorted([...visited].map((filePath) => toRepoRelative(repoRoot, filePath))),
  };
}

function discoverLazyImports({ repoRoot }) {
  const sourceRoot = path.join(repoRoot, 'web', 'app', 'src');
  const entries = [];

  for (const filePath of listSourceFiles(sourceRoot)) {
    const source = readText(filePath);
    const stripped = stripComments(source);
    LAZY_IMPORT_PATTERN.lastIndex = 0;
    let match = LAZY_IMPORT_PATTERN.exec(stripped);

    while (match) {
      entries.push({
        source: toRepoRelative(repoRoot, filePath),
        sourceLine: lineNumberAt(stripped, match.index),
        specifier: match[1],
        resolvedPath: resolveSourceFile(filePath, match[1]),
      });
      match = LAZY_IMPORT_PATTERN.exec(stripped);
    }
  }

  return entries.sort((left, right) =>
    `${left.source}:${left.specifier}`.localeCompare(`${right.source}:${right.specifier}`)
  );
}

function extractDelimitedContent(source, start, open, close) {
  if (source[start] !== open) {
    return null;
  }

  let depth = 0;
  let quote = null;

  for (let index = start; index < source.length; index += 1) {
    const character = source[index];

    if (quote) {
      if (character === '\\') {
        index += 1;
      } else if (character === quote) {
        quote = null;
      }
      continue;
    }

    if (character === '"' || character === "'" || character === '`') {
      quote = character;
      continue;
    }

    if (character === open) {
      depth += 1;
    } else if (character === close) {
      depth -= 1;
      if (depth === 0) {
        return source.slice(start + 1, index);
      }
    }
  }

  return null;
}

function parseOptimizeDepsEntries(viteConfigSource, property) {
  const optimizeDepsMatch = /optimizeDeps\s*:\s*\{/u.exec(viteConfigSource);
  if (!optimizeDepsMatch) {
    return [];
  }

  const optimizeDepsBody = extractDelimitedContent(
    viteConfigSource,
    optimizeDepsMatch.index + optimizeDepsMatch[0].length - 1,
    '{',
    '}'
  );
  if (!optimizeDepsBody) {
    return [];
  }

  const propertyMatch = new RegExp(`\\b${property}\\s*:\\s*\\[`, 'u').exec(optimizeDepsBody);
  if (!propertyMatch) {
    return [];
  }

  const propertyBody = extractDelimitedContent(
    optimizeDepsBody,
    propertyMatch.index + propertyMatch[0].length - 1,
    '[',
    ']'
  );
  if (!propertyBody) {
    return [];
  }

  const entries = [];
  const stringPattern = /['"]([^'"]+)['"]/gu;
  let match = stringPattern.exec(propertyBody);

  while (match) {
    entries.push(match[1]);
    match = stringPattern.exec(propertyBody);
  }

  return uniqueSorted(entries);
}

function parseOptimizeDepsInclude(viteConfigSource) {
  return parseOptimizeDepsEntries(viteConfigSource, 'include');
}

function parseOptimizeDepsExclude(viteConfigSource) {
  return parseOptimizeDepsEntries(viteConfigSource, 'exclude');
}

function loadManifest(manifestPath) {
  const parsed = JSON.parse(readText(manifestPath));
  if (!Array.isArray(parsed.entries)) {
    throw new Error(`Vite lazy deps manifest must contain an entries array: ${manifestPath}`);
  }

  return parsed;
}

function manifestKey({ source, specifier }) {
  return `${source}\0${specifier}`;
}

function requiresSmokeManifest(entry) {
  return /\/app\/router\.tsx$/u.test(`/${entry.source}`)
    || /\/features\/[^/]+\/pages\//u.test(`/${entry.source}`);
}

function validateManifestEntry(entry) {
  const findings = [];

  if (!Array.isArray(entry.smokePaths) || entry.smokePaths.length === 0) {
    findings.push({
      code: 'invalid-smoke-manifest',
      source: entry.source,
      specifier: entry.specifier,
      message: 'Manifest entry must list at least one smoke path.',
    });
    return findings;
  }

  for (const smokePath of entry.smokePaths) {
    const pathValue = typeof smokePath === 'string' ? smokePath : smokePath.path;
    if (typeof pathValue !== 'string' || !pathValue.startsWith('/') || pathValue.includes('$')) {
      findings.push({
        code: 'invalid-smoke-manifest',
        source: entry.source,
        specifier: entry.specifier,
        smokePath: pathValue,
        message: 'Smoke paths must be concrete app paths starting with /.',
      });
    }
  }

  return findings;
}

function analyzeStaticLazyDeps({
  repoRoot = getRepoRoot(),
  manifestPath = DEFAULT_MANIFEST_PATH,
} = {}) {
  const packageInfo = readWebPackageInfo(repoRoot);
  const mainEntry = path.join(repoRoot, 'web', 'app', 'src', 'main.tsx');
  const viteConfigPath = path.join(repoRoot, 'web', 'app', 'vite.config.ts');
  const viteConfigSource = readText(viteConfigPath);
  const optimizeDepsInclude = parseOptimizeDepsInclude(viteConfigSource);
  const optimizeDepsExclude = parseOptimizeDepsExclude(viteConfigSource);
  const optimizedRoots = new Set(optimizeDepsInclude.map(packageRoot).filter(Boolean));
  const excludedRoots = new Set(optimizeDepsExclude.map(packageRoot).filter(Boolean));
  const eagerGraph = collectModuleGraphDependencies({
    entryFiles: [mainEntry],
    packageInfo,
    repoRoot,
    includeDynamicImports: false,
  });
  const eagerDependencies = new Set(eagerGraph.dependencies.keys());
  const lazyEntries = discoverLazyImports({ repoRoot });
  const lazyDependencies = new Map();
  const findings = [];

  for (const entry of lazyEntries) {
    if (isBareNpmImport(entry.specifier, packageInfo)) {
      addDependency(
        lazyDependencies,
        entry.specifier,
        path.join(repoRoot, entry.source),
        repoRoot,
        packageInfo
      );
      continue;
    }

    if (!entry.resolvedPath) {
      findings.push({
        code: 'unresolved-lazy-import',
        source: entry.source,
        line: entry.sourceLine,
        specifier: entry.specifier,
        message: 'Lazy import target could not be resolved.',
      });
      continue;
    }

    const graph = collectModuleGraphDependencies({
      entryFiles: [entry.resolvedPath],
      packageInfo,
      repoRoot,
      includeDynamicImports: true,
    });

    for (const [dependency, files] of graph.dependencies.entries()) {
      if (!lazyDependencies.has(dependency)) {
        lazyDependencies.set(dependency, new Set());
      }

      for (const filePath of files) {
        lazyDependencies.get(dependency).add(filePath);
      }
    }
  }

  const lazyOnlyDependencies = uniqueSorted(
    [...lazyDependencies.keys()].filter((dependency) => !eagerDependencies.has(dependency))
  );

  for (const dependency of lazyOnlyDependencies) {
    if (!optimizedRoots.has(dependency) && !excludedRoots.has(dependency)) {
      findings.push({
        code: 'missing-optimize-dep',
        dependency,
        files: uniqueSorted([...lazyDependencies.get(dependency)]),
        message: `Lazy-only dependency "${dependency}" is not listed in web/app/vite.config.ts optimizeDeps.include.`,
      });
    }
  }

  const manifest = loadManifest(manifestPath);
  const manifestEntries = new Map(manifest.entries.map((entry) => [manifestKey(entry), entry]));

  for (const entry of lazyEntries.filter(requiresSmokeManifest)) {
    if (!manifestEntries.has(manifestKey(entry))) {
      findings.push({
        code: 'missing-smoke-manifest',
        source: entry.source,
        line: entry.sourceLine,
        specifier: entry.specifier,
        message: 'Route-owned lazy import is missing vite-lazy-deps-gate manifest smoke coverage.',
      });
    }
  }

  for (const entry of manifest.entries) {
    findings.push(...validateManifestEntry(entry));
  }

  return {
    ok: findings.length === 0,
    findings,
    lazyEntries: lazyEntries.map((entry) => ({
      source: entry.source,
      line: entry.sourceLine,
      specifier: entry.specifier,
      resolved: entry.resolvedPath ? toRepoRelative(repoRoot, entry.resolvedPath) : null,
      requiresSmokeManifest: requiresSmokeManifest(entry),
    })),
    lazyOnlyDependencies,
    optimizeDepsInclude,
    optimizeDepsExclude,
  };
}

function detectRuntimeFailureSignals({ responses = [], consoleMessages = [], pageErrors = [] } = {}) {
  const signals = [];

  for (const response of responses) {
    const statusText = response.statusText || '';
    const url = response.url || '';
    if (
      response.status === 504
      && (/Outdated Optimize Dep/u.test(statusText) || /Outdated Optimize Dep/u.test(url))
    ) {
      signals.push({
        code: 'outdated-optimize-dep',
        url,
        status: response.status,
        statusText,
      });
    }
  }

  for (const message of [...consoleMessages, ...pageErrors]) {
    if (/Failed to fetch dynamically imported module/u.test(message)) {
      signals.push({
        code: 'dynamic-import-fetch-failed',
        message,
      });
    }
  }

  return signals;
}

function flattenSmokePaths(manifest, includeFixtureRoutes = false) {
  const paths = [];

  for (const entry of manifest.entries) {
    for (const smokePath of entry.smokePaths || []) {
      const pathValue = typeof smokePath === 'string' ? smokePath : smokePath.path;
      const requiresFixture = typeof smokePath === 'object' && smokePath.requiresFixture === true;
      if (requiresFixture && !includeFixtureRoutes) {
        continue;
      }
      if (typeof pathValue === 'string') {
        paths.push(pathValue);
      }
    }
  }

  return uniqueSorted(paths);
}

function writeRuntimeSmokeReports({
  repoRoot,
  env = process.env,
  result,
}) {
  const outputDir = resolveOutputDir(repoRoot, env);
  fs.mkdirSync(outputDir, { recursive: true });

  const jsonPath = path.join(outputDir, 'vite-lazy-deps-gate-smoke.json');
  const markdownPath = path.join(outputDir, 'vite-lazy-deps-gate-smoke.md');
  const report = {
    status: result.ok ? 'passed' : 'failed',
    smokePaths: result.smokePaths,
    signals: result.signals,
    responseCount: result.responseCount,
    consoleErrorCount: result.consoleErrorCount,
    pageErrorCount: result.pageErrorCount,
  };
  const markdown = [
    '# Vite Lazy Deps Gate Smoke',
    '',
    `- Status: ${report.status}`,
    `- Smoke paths: ${report.smokePaths.length}`,
    `- Signals: ${report.signals.length}`,
    `- JSON: ${normalizeRepoPath(path.relative(repoRoot, jsonPath))}`,
    '',
  ].join('\n');

  fs.writeFileSync(jsonPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  fs.writeFileSync(markdownPath, markdown, 'utf8');

  return {
    jsonPath,
    markdownPath,
    report,
  };
}

function resolveTargetUrl(webBaseUrl, targetPath) {
  const base = webBaseUrl.replace(/\/$/u, '');
  return targetPath.startsWith('http://') || targetPath.startsWith('https://')
    ? targetPath
    : `${base}${targetPath.startsWith('/') ? targetPath : `/${targetPath}`}`;
}

async function defaultVisitSmokePath({
  page,
  webBaseUrl,
  targetPath,
  timeout,
}) {
  await page.goto(resolveTargetUrl(webBaseUrl, targetPath), {
    waitUntil: 'domcontentloaded',
    timeout,
  });
  await page.waitForLoadState('networkidle', { timeout }).catch(() => {});
}

async function runRuntimeSmoke({
  repoRoot = getRepoRoot(),
  manifestPath = DEFAULT_MANIFEST_PATH,
  webBaseUrl = DEFAULT_WEB_BASE_URL,
  timeout = DEFAULT_TIMEOUT,
  headless = true,
  storageStatePath = null,
  includeFixtureRoutes = false,
  playwright = null,
  visitSmokePathImpl = defaultVisitSmokePath,
  env = process.env,
} = {}) {
  const manifest = loadManifest(manifestPath);
  const smokePaths = flattenSmokePaths(manifest, includeFixtureRoutes);
  const runtime = playwright || loadPlaywright(repoRoot);
  const browser = await runtime.chromium.launch({ headless });
  const responses = [];
  const consoleMessages = [];
  const pageErrors = [];

  try {
    const contextOptions = storageStatePath ? { storageState: storageStatePath } : {};
    const context = await browser.newContext(contextOptions);
    const page = await context.newPage();

    page.on('response', (response) => {
      responses.push({
        url: response.url(),
        status: response.status(),
        statusText: response.statusText(),
      });
    });
    page.on('console', (message) => {
      if (message.type() === 'error') {
        consoleMessages.push(message.text());
      }
    });
    page.on('pageerror', (error) => {
      pageErrors.push(error.message || String(error));
    });

    for (const targetPath of smokePaths) {
      await visitSmokePathImpl({
        page,
        webBaseUrl,
        targetPath,
        timeout,
      });
    }

    const signals = detectRuntimeFailureSignals({
      responses,
      consoleMessages,
      pageErrors,
    });

    const result = {
      ok: signals.length === 0,
      smokePaths,
      signals,
      responseCount: responses.length,
      consoleErrorCount: consoleMessages.length,
      pageErrorCount: pageErrors.length,
    };
    writeRuntimeSmokeReports({ repoRoot, env, result });

    return result;
  } finally {
    await browser.close();
  }
}

function parseCliArgs(argv = []) {
  const options = {
    help: false,
    smoke: false,
    manifestPath: DEFAULT_MANIFEST_PATH,
    webBaseUrl: DEFAULT_WEB_BASE_URL,
    timeout: DEFAULT_TIMEOUT,
    headless: true,
    storageStatePath: null,
    includeFixtureRoutes: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '-h' || arg === '--help') {
      options.help = true;
      continue;
    }

    if (arg === '--smoke') {
      options.smoke = true;
      continue;
    }

    if (arg === '--headed') {
      options.headless = false;
      continue;
    }

    if (arg === '--include-fixture-routes') {
      options.includeFixtureRoutes = true;
      continue;
    }

    if (arg === '--manifest' || arg === '--web-base-url' || arg === '--timeout' || arg === '--storage-state') {
      const next = argv[index + 1];
      if (!next) {
        throw new Error(`${arg} requires a value`);
      }

      if (arg === '--manifest') {
        options.manifestPath = next;
      } else if (arg === '--web-base-url') {
        options.webBaseUrl = next;
      } else if (arg === '--timeout') {
        options.timeout = Number.parseInt(next, 10);
        if (!Number.isFinite(options.timeout) || options.timeout <= 0) {
          throw new Error('--timeout must be a positive integer');
        }
      } else {
        options.storageStatePath = next;
      }

      index += 1;
      continue;
    }

    throw new Error(`Unknown vite-lazy-deps-gate option: ${arg}`);
  }

  return options;
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(
    'Usage: node scripts/node/vite-lazy-deps-gate.js [--manifest <path>] [--smoke] [--web-base-url <url>] [--storage-state <path>] [--include-fixture-routes]\n'
  );
}

function formatStaticSummary(result) {
  return [
    `[vite-lazy-deps-gate] static ${result.ok ? 'passed' : 'failed'}`,
    `lazy imports: ${result.lazyEntries.length}`,
    `lazy-only dependencies: ${result.lazyOnlyDependencies.length ? result.lazyOnlyDependencies.join(', ') : 'none'}`,
  ].join('\n') + '\n';
}

function formatFindings(findings) {
  return findings.map((finding) => {
    const location = finding.source
      ? `${finding.source}${finding.line ? `:${finding.line}` : ''}`
      : finding.dependency || 'runtime';
    return `[vite-lazy-deps-gate] ${finding.code} ${location}: ${finding.message}\n`;
  }).join('');
}

async function main(argv = [], deps = {}) {
  const options = parseCliArgs(argv);
  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));
  const writeStderr = deps.writeStderr || ((text) => process.stderr.write(text));

  if (options.help) {
    usage(writeStdout);
    return 0;
  }

  const repoRoot = deps.repoRoot || getRepoRoot();
  const staticResult = analyzeStaticLazyDeps({
    repoRoot,
    manifestPath: options.manifestPath,
  });
  writeStdout(formatStaticSummary(staticResult));

  if (!staticResult.ok) {
    writeStderr(formatFindings(staticResult.findings));
    return 1;
  }

  if (!options.smoke) {
    return 0;
  }

  const smokeResult = await (deps.runRuntimeSmokeImpl || runRuntimeSmoke)({
    repoRoot,
    manifestPath: options.manifestPath,
    webBaseUrl: options.webBaseUrl,
    timeout: options.timeout,
    headless: options.headless,
    storageStatePath: options.storageStatePath,
    includeFixtureRoutes: options.includeFixtureRoutes,
    playwright: deps.playwright,
    visitSmokePathImpl: deps.visitSmokePathImpl,
    env: deps.env || process.env,
  });

  if (!smokeResult.ok) {
    writeStderr(formatFindings(smokeResult.signals));
    return 1;
  }

  writeStdout(`[vite-lazy-deps-gate] smoke passed: ${smokeResult.smokePaths.length} path(s)\n`);
  return 0;
}

module.exports = {
  DEFAULT_MANIFEST_PATH,
  analyzeStaticLazyDeps,
  collectModuleGraphDependencies,
  detectRuntimeFailureSignals,
  discoverLazyImports,
  flattenSmokePaths,
  main,
  packageRoot,
  parseCliArgs,
  parseOptimizeDepsInclude,
  runRuntimeSmoke,
  writeRuntimeSmokeReports,
};
