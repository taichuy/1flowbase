const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');

const {
  buildChromiumLaunchOptions,
  createRunArtifacts,
  createSuccessResult,
  parseCliArgs,
  resolveTargetUrl,
  runPageDebug,
} = require('../core.js');

test('parseCliArgs defaults to snapshot mode for a bare route', () => {
  assert.deepEqual(parseCliArgs(['/settings']), {
    help: false,
    mode: 'snapshot',
    target: '/settings',
    webBaseUrl: 'http://127.0.0.1:3100',
    apiBaseUrl: 'http://127.0.0.1:7800',
    outDir: null,
    headless: true,
    timeout: 15000,
    account: null,
    password: null,
    waitForSelector: null,
    waitForUrl: null,
  });
});

test('createRunArtifacts allocates the expected files for snapshot mode', () => {
  const repoRoot = '/repo';
  const artifacts = createRunArtifacts({
    repoRoot,
    mode: 'snapshot',
    outDir: null,
    now: new Date('2026-04-18T12:34:56Z'),
  });

  assert.equal(
    artifacts.runDir,
    path.join(repoRoot, 'tmp', 'page-debug', '2026-04-18T12-34-56-000Z')
  );
  assert.equal(artifacts.storageStatePath, path.join(artifacts.runDir, 'storage-state.json'));
  assert.equal(artifacts.metaPath, path.join(artifacts.runDir, 'meta.json'));
  assert.equal(artifacts.htmlPath, path.join(artifacts.runDir, 'index.html'));
  assert.equal(artifacts.screenshotPath, path.join(artifacts.runDir, 'page.png'));
  assert.equal(artifacts.consoleLogPath, path.join(artifacts.runDir, 'console.ndjson'));
});

test('resolveTargetUrl expands relative routes against the configured web base url', () => {
  assert.equal(
    resolveTargetUrl('http://127.0.0.1:3100', '/me/profile'),
    'http://127.0.0.1:3100/me/profile'
  );
  assert.equal(
    resolveTargetUrl('http://127.0.0.1:3100', 'http://127.0.0.1:3100/settings/members'),
    'http://127.0.0.1:3100/settings/members'
  );
});

test('createSuccessResult exposes machine-readable artifact paths', () => {
  assert.deepEqual(
    createSuccessResult({
      mode: 'snapshot',
      requestedUrl: '/settings',
      finalUrl: 'http://127.0.0.1:3100/settings/members',
      authenticated: true,
      readyState: 'ready_with_selector',
      warnings: [],
      artifacts: {
        runDir: '/tmp/page-debug/run-1',
        metaPath: '/tmp/page-debug/run-1/meta.json',
        storageStatePath: '/tmp/page-debug/run-1/storage-state.json',
        htmlPath: '/tmp/page-debug/run-1/index.html',
        screenshotPath: '/tmp/page-debug/run-1/page.png',
        consoleLogPath: '/tmp/page-debug/run-1/console.ndjson',
      },
    }),
    {
      ok: true,
      mode: 'snapshot',
      requestedUrl: '/settings',
      finalUrl: 'http://127.0.0.1:3100/settings/members',
      authenticated: true,
      readyState: 'ready_with_selector',
      outputDir: '/tmp/page-debug/run-1',
      metaPath: '/tmp/page-debug/run-1/meta.json',
      storageStatePath: '/tmp/page-debug/run-1/storage-state.json',
      htmlPath: '/tmp/page-debug/run-1/index.html',
      screenshotPath: '/tmp/page-debug/run-1/page.png',
      consoleLogPath: '/tmp/page-debug/run-1/console.ndjson',
      warnings: [],
    }
  );
});

test('buildChromiumLaunchOptions uses configured system chrome executable', () => {
  assert.deepEqual(
    buildChromiumLaunchOptions({
      headless: true,
      env: {
        PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH: '/usr/bin/google-chrome',
      },
    }),
    {
      headless: true,
      executablePath: '/usr/bin/google-chrome',
    }
  );
});

test('runPageDebug login mode returns structured json without launching a browser', async () => {
  const writes = [];
  const result = await runPageDebug(
    {
      help: false,
      mode: 'login',
      target: null,
      webBaseUrl: 'http://127.0.0.1:3100',
      apiBaseUrl: 'http://127.0.0.1:7800',
      outDir: null,
      headless: true,
      timeout: 15000,
      account: 'root',
      password: 'change-me',
      waitForSelector: null,
      waitForUrl: null,
    },
    {
      repoRoot: '/repo',
      playwright: {
        request: {
          newContext: async () => ({
            post: async () => ({ ok: () => true, status: () => 200, json: async () => ({ data: {} }) }),
            storageState: async () => {},
            dispose: async () => {},
          }),
        },
      },
      loadRootCredentials: () => ({
        account: 'root',
        password: 'change-me',
        envFilePath: '/repo/.env',
      }),
      writeStdoutJson: (payload) => writes.push(payload),
    }
  );

  assert.equal(result.ok, true);
  assert.equal(result.mode, 'login');
  assert.equal(result.outputDir, null);
  assert.equal(writes[0].mode, 'login');
});
