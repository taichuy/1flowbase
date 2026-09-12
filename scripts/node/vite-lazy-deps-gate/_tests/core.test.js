const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  analyzeStaticLazyDeps,
  detectRuntimeFailureSignals,
  main,
  writeRuntimeSmokeReports,
} = require('../core.js');

function writeFile(repoRoot, relativePath, content) {
  const absolutePath = path.join(repoRoot, relativePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, content, 'utf8');
}

function createFixtureRepo({
  optimizeDepsInclude = [],
  manifestEntries = [],
  lazyPageImport = "import PresentLazy from 'present-lazy';\n",
} = {}) {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-vite-lazy-deps-'));
  writeFile(
    repoRoot,
    'web/app/package.json',
    JSON.stringify({
      dependencies: {
        'eager-shared': '^1.0.0',
        'missing-lazy': '^1.0.0',
        'present-lazy': '^1.0.0',
      },
      devDependencies: {},
    }, null, 2)
  );
  writeFile(
    repoRoot,
    'web/app/vite.config.ts',
    [
      'export default {',
      '  optimizeDeps: {',
      `    include: [${optimizeDepsInclude.map((entry) => `'${entry}'`).join(', ')}]`,
      '  }',
      '};',
    ].join('\n')
  );
  writeFile(
    repoRoot,
    'web/app/src/main.tsx',
    [
      "import 'eager-shared';",
      "import { App } from './app/App';",
      'App();',
    ].join('\n')
  );
  writeFile(
    repoRoot,
    'web/app/src/app/App.tsx',
    [
      "import { AppRouterProvider } from './router';",
      'export function App() { return AppRouterProvider(); }',
    ].join('\n')
  );
  writeFile(
    repoRoot,
    'web/app/src/app/router.tsx',
    [
      "import { lazy } from 'react';",
      "const LazyPage = lazy(() => import('../features/lazy/LazyPage'));",
      "const lazyRoute = createRoute({ path: '/lazy/$itemId', component: LazyPage });",
      'export function AppRouterProvider() { return lazyRoute; }',
    ].join('\n')
  );
  writeFile(
    repoRoot,
    'web/app/src/features/lazy/LazyPage.tsx',
    [
      lazyPageImport,
      'export function LazyPage() { return null; }',
    ].join('\n')
  );
  const manifestPath = path.join(repoRoot, 'scripts/node/vite-lazy-deps-gate/manifest.json');
  writeFile(
    repoRoot,
    'scripts/node/vite-lazy-deps-gate/manifest.json',
    JSON.stringify({ version: 1, entries: manifestEntries }, null, 2)
  );

  return { repoRoot, manifestPath };
}

test('analyzeStaticLazyDeps passes when lazy-only dependencies are listed in optimizeDeps.include', () => {
  const { repoRoot, manifestPath } = createFixtureRepo({
    optimizeDepsInclude: ['present-lazy'],
    manifestEntries: [
      {
        source: 'web/app/src/app/router.tsx',
        specifier: '../features/lazy/LazyPage',
        smokePaths: ['/lazy/example'],
      },
    ],
  });

  const result = analyzeStaticLazyDeps({ repoRoot, manifestPath });

  assert.equal(result.ok, true);
  assert.deepEqual(result.findings, []);
  assert.deepEqual(result.lazyOnlyDependencies, ['present-lazy']);
});

test('analyzeStaticLazyDeps reads optimizeDeps.include after nested options', () => {
  const { repoRoot, manifestPath } = createFixtureRepo({
    optimizeDepsInclude: ['present-lazy'],
    manifestEntries: [
      {
        source: 'web/app/src/app/router.tsx',
        specifier: '../features/lazy/LazyPage',
        smokePaths: ['/lazy/example'],
      },
    ],
  });
  writeFile(
    repoRoot,
    'web/app/vite.config.ts',
    [
      'export default {',
      '  optimizeDeps: {',
      '    rolldownOptions: {',
      '      output: { minify: true }',
      '    },',
      "    include: ['present-lazy']",
      '  }',
      '};',
    ].join('\n')
  );

  const result = analyzeStaticLazyDeps({ repoRoot, manifestPath });

  assert.equal(result.ok, true);
  assert.deepEqual(result.findings, []);
});

test('analyzeStaticLazyDeps fails when a lazy-only bare npm import is not optimized', () => {
  const { repoRoot, manifestPath } = createFixtureRepo({
    manifestEntries: [
      {
        source: 'web/app/src/app/router.tsx',
        specifier: '../features/lazy/LazyPage',
        smokePaths: ['/lazy/example'],
      },
    ],
    lazyPageImport: "import MissingLazy from 'missing-lazy';\n",
  });

  const result = analyzeStaticLazyDeps({ repoRoot, manifestPath });

  assert.equal(result.ok, false);
  assert.deepEqual(result.findings.map((finding) => finding.code), [
    'missing-optimize-dep',
  ]);
  assert.equal(result.findings[0].dependency, 'missing-lazy');
});

test('analyzeStaticLazyDeps accepts a lazy-only dependency explicitly excluded by Vite', () => {
  const { repoRoot, manifestPath } = createFixtureRepo({
    manifestEntries: [
      {
        source: 'web/app/src/app/router.tsx',
        specifier: '../features/lazy/LazyPage',
        smokePaths: ['/lazy/example'],
      },
    ],
  });
  writeFile(
    repoRoot,
    'web/app/vite.config.ts',
    [
      'export default {',
      '  optimizeDeps: {',
      "    exclude: ['present-lazy'],",
      '    include: []',
      '  }',
      '};',
    ].join('\n')
  );

  const result = analyzeStaticLazyDeps({ repoRoot, manifestPath });

  assert.equal(result.ok, true);
  assert.deepEqual(result.findings, []);
});

test('analyzeStaticLazyDeps does not require optimizeDeps for dependencies already loaded eagerly', () => {
  const { repoRoot, manifestPath } = createFixtureRepo({
    manifestEntries: [
      {
        source: 'web/app/src/app/router.tsx',
        specifier: '../features/lazy/LazyPage',
        smokePaths: ['/lazy/example'],
      },
    ],
    lazyPageImport: "import EagerShared from 'eager-shared';\n",
  });

  const result = analyzeStaticLazyDeps({ repoRoot, manifestPath });

  assert.equal(result.ok, true);
  assert.deepEqual(result.findings, []);
  assert.deepEqual(result.lazyOnlyDependencies, []);
});

test('analyzeStaticLazyDeps treats local @1flowbase aliases as workspace imports', () => {
  const { repoRoot, manifestPath } = createFixtureRepo({
    manifestEntries: [
      {
        source: 'web/app/src/app/router.tsx',
        specifier: '../features/lazy/LazyPage',
        smokePaths: ['/lazy/example'],
      },
    ],
    lazyPageImport: "import ContractFixtures from '@1flowbase/model-provider-contracts';\n",
  });

  const result = analyzeStaticLazyDeps({ repoRoot, manifestPath });

  assert.equal(result.ok, true);
  assert.deepEqual(result.findings, []);
  assert.deepEqual(result.lazyOnlyDependencies, []);
});

test('analyzeStaticLazyDeps ignores virtual and interpolated lazy graph specifiers', () => {
  const { repoRoot, manifestPath } = createFixtureRepo({
    manifestEntries: [
      {
        source: 'web/app/src/app/router.tsx',
        specifier: '../features/lazy/LazyPage',
        smokePaths: ['/lazy/example'],
      },
    ],
    lazyPageImport: [
      "import 'virtual:generated-module';",
      "const moduleName = import('${packageRoot}/generated-module');",
    ].join('\n'),
  });

  const result = analyzeStaticLazyDeps({ repoRoot, manifestPath });

  assert.equal(result.ok, true);
  assert.deepEqual(result.lazyOnlyDependencies, []);
});

test('analyzeStaticLazyDeps reports lazy route imports missing smoke manifest coverage', () => {
  const { repoRoot, manifestPath } = createFixtureRepo({
    optimizeDepsInclude: ['present-lazy'],
    manifestEntries: [],
  });

  const result = analyzeStaticLazyDeps({ repoRoot, manifestPath });

  assert.equal(result.ok, false);
  assert.deepEqual(result.findings.map((finding) => finding.code), [
    'missing-smoke-manifest',
  ]);
  assert.equal(result.findings[0].source, 'web/app/src/app/router.tsx');
  assert.equal(result.findings[0].specifier, '../features/lazy/LazyPage');
});

test('detectRuntimeFailureSignals catches Vite dev optimized dep failures', () => {
  const signals = detectRuntimeFailureSignals({
    responses: [
      {
        url: 'http://127.0.0.1:3100/node_modules/.vite/deps/chunk.js?v=old',
        status: 504,
        statusText: 'Outdated Optimize Dep',
      },
    ],
    consoleMessages: [
      'TypeError: Failed to fetch dynamically imported module',
    ],
    pageErrors: [],
  });

  assert.deepEqual(signals.map((signal) => signal.code), [
    'outdated-optimize-dep',
    'dynamic-import-fetch-failed',
  ]);
});

test('writeRuntimeSmokeReports stores smoke evidence under tmp/test-governance', () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-vite-lazy-report-'));
  const result = {
    ok: false,
    smokePaths: ['/settings/docs'],
    signals: [
      {
        code: 'dynamic-import-fetch-failed',
        message: 'Failed to fetch dynamically imported module',
      },
    ],
    responseCount: 2,
    consoleErrorCount: 1,
    pageErrorCount: 0,
  };

  const written = writeRuntimeSmokeReports({ repoRoot, result });

  assert.equal(
    written.jsonPath,
    path.join(repoRoot, 'tmp/test-governance/vite-lazy-deps-gate-smoke.json')
  );
  assert.equal(fs.existsSync(written.markdownPath), true);
  const report = JSON.parse(fs.readFileSync(written.jsonPath, 'utf8'));
  assert.equal(report.status, 'failed');
  assert.deepEqual(report.smokePaths, ['/settings/docs']);
  assert.equal(report.signals[0].code, 'dynamic-import-fetch-failed');
});

test('main runs the static gate and returns failure for missing lazy optimizeDeps', async () => {
  const { repoRoot, manifestPath } = createFixtureRepo({
    manifestEntries: [
      {
        source: 'web/app/src/app/router.tsx',
        specifier: '../features/lazy/LazyPage',
        smokePaths: ['/lazy/example'],
      },
    ],
    lazyPageImport: "import MissingLazy from 'missing-lazy';\n",
  });
  let stderr = '';

  const status = await main(['--manifest', manifestPath], {
    repoRoot,
    writeStdout() {},
    writeStderr(text) {
      stderr += text;
    },
  });

  assert.equal(status, 1);
  assert.match(stderr, /missing-optimize-dep/u);
  assert.match(stderr, /missing-lazy/u);
});
