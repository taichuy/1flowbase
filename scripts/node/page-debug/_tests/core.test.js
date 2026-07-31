const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
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
  let revoked = false;
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
            post: async () => ({
              ok: () => true,
              status: () => 200,
              json: async () => ({ data: { csrf_token: 'csrf-token' } }),
            }),
            storageState: async () => {},
            delete: async () => {
              revoked = true;
              return { ok: () => true, status: () => 204 };
            },
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
  assert.equal(revoked, true);
});

test('AC-001 runPageDebug snapshot revokes its temporary session after evidence capture', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-page-debug-'));
  let browserClosed = false;
  let sessionDisposed = false;
  const finalUrl = 'http://127.0.0.1:3100/settings';

  const result = await runPageDebug(
    pageDebugOptions({ mode: 'snapshot', target: '/settings' }),
    {
      repoRoot,
      playwright: {
        chromium: {
          launch: async () => ({
            newContext: async () => ({ newPage: async () => fakeReadyPage(finalUrl) }),
            close: async () => {
              browserClosed = true;
            },
          }),
        },
      },
      openTemporaryConsoleSession: async () => ({
        dispose: async () => {
          sessionDisposed = true;
        },
      }),
      loadRootCredentials: rootCredentials,
      writeStdoutJson: () => {},
    }
  );

  assert.equal(result.ok, true);
  assert.equal(browserClosed, true);
  assert.equal(sessionDisposed, true);
});

test('AC-002 runPageDebug revokes its temporary session when browser launch fails', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-page-debug-'));
  let sessionDisposed = false;

  await assert.rejects(
    () =>
      runPageDebug(pageDebugOptions({ mode: 'snapshot', target: '/settings' }), {
        repoRoot,
        playwright: {
          chromium: {
            launch: async () => {
              throw new Error('browser unavailable');
            },
          },
        },
        openTemporaryConsoleSession: async () => ({
          dispose: async () => {
            sessionDisposed = true;
          },
        }),
        loadRootCredentials: rootCredentials,
        writeStdoutJson: () => {},
      }),
    /browser unavailable/u
  );

  assert.equal(sessionDisposed, true);
});

test('AC-001 runPageDebug open mode keeps the session until the browser disconnects', async () => {
  const repoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-page-debug-'));
  let disconnectBrowser = null;
  let sessionDisposed = false;
  const finalUrl = 'http://127.0.0.1:3100/settings';

  const result = await runPageDebug(pageDebugOptions({ mode: 'open', target: '/settings' }), {
    repoRoot,
    playwright: {
      chromium: {
        launch: async () => ({
          once: (_event, listener) => {
            disconnectBrowser = listener;
          },
          newContext: async () => ({ newPage: async () => fakeReadyPage(finalUrl) }),
          close: async () => {
            throw new Error('open mode must not close the browser eagerly');
          },
        }),
      },
    },
    openTemporaryConsoleSession: async () => ({
      dispose: async () => {
        sessionDisposed = true;
      },
    }),
    loadRootCredentials: rootCredentials,
    writeStdoutJson: () => {},
  });

  assert.equal(result.ok, true);
  assert.equal(sessionDisposed, false);
  assert.equal(typeof disconnectBrowser, 'function');
  disconnectBrowser();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(sessionDisposed, true);
});

function pageDebugOptions({ mode, target }) {
  return {
    help: false,
    mode,
    target,
    webBaseUrl: 'http://127.0.0.1:3100',
    apiBaseUrl: 'http://127.0.0.1:7800',
    outDir: null,
    headless: true,
    timeout: 15000,
    account: 'root',
    password: 'change-me',
    waitForSelector: null,
    waitForUrl: null,
  };
}

function rootCredentials() {
  return {
    account: 'root',
    password: 'change-me',
    envFilePath: '/repo/.env',
  };
}

function fakeReadyPage(finalUrl) {
  return {
    on: () => {},
    goto: async () => {},
    waitForLoadState: async () => {},
    waitForFunction: async () => {},
    url: () => finalUrl,
    screenshot: async () => {},
    evaluate: async () => ({
      html: '<!doctype html><html><body>ready</body></html>',
      inlineStyles: [],
      inlineScripts: [],
    }),
  };
}
