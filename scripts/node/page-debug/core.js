const fs = require('node:fs');
const path = require('node:path');
const { createRequire } = require('node:module');

const { loadRootCredentials, loginAndPersistStorageState } = require('./auth.js');
const { createConsoleCollector, writeEvidence } = require('./evidence.js');
const { waitForPageReady } = require('./readiness.js');
const {
  INLINE_SCRIPT_PREFIX,
  INLINE_STYLE_PREFIX,
  assignInlineArtifactPaths,
  assignLocalResourcePaths,
  buildMetaPayload,
  rewriteSnapshotHtml,
  writeSnapshotArtifacts,
} = require('./snapshot.js');

const DEFAULT_WEB_BASE_URL = 'http://127.0.0.1:3100';
const DEFAULT_API_BASE_URL = 'http://127.0.0.1:7800';
const DEFAULT_TIMEOUT = 15000;
const CHROMIUM_EXECUTABLE_ENV = 'PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH';
const MODES = new Set(['snapshot', 'open', 'login']);

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function parseCliArgs(argv) {
  const options = {
    help: false,
    mode: 'snapshot',
    target: null,
    webBaseUrl: DEFAULT_WEB_BASE_URL,
    apiBaseUrl: DEFAULT_API_BASE_URL,
    outDir: null,
    headless: true,
    timeout: DEFAULT_TIMEOUT,
    account: null,
    password: null,
    waitForSelector: null,
    waitForUrl: null,
  };

  const args = [...argv];
  if (args.includes('-h') || args.includes('--help')) {
    return { ...options, help: true };
  }

  if (args[0] && MODES.has(args[0])) {
    options.mode = args.shift();
  }

  if (options.mode !== 'login' && args[0] && !args[0].startsWith('--')) {
    options.target = args.shift();
  }

  while (args.length > 0) {
    const arg = args.shift();
    const value = args[0];

    if (arg === '--web-base-url') {
      options.webBaseUrl = args.shift();
    } else if (arg === '--api-base-url') {
      options.apiBaseUrl = args.shift();
    } else if (arg === '--out-dir') {
      options.outDir = args.shift();
    } else if (arg === '--headless') {
      options.headless = value !== 'false' ? true : (args.shift(), false);
    } else if (arg === '--timeout') {
      options.timeout = Number.parseInt(args.shift(), 10);
    } else if (arg === '--account') {
      options.account = args.shift();
    } else if (arg === '--password') {
      options.password = args.shift();
    } else if (arg === '--wait-for-selector') {
      options.waitForSelector = args.shift();
    } else if (arg === '--wait-for-url') {
      options.waitForUrl = args.shift();
    } else {
      throw new Error(`未知参数：${arg}`);
    }
  }

  if (options.mode !== 'login' && !options.target) {
    throw new Error(`模式 ${options.mode} 需要提供目标路由或 URL`);
  }

  if (options.mode === 'open' && argv.includes('--headless') === false) {
    options.headless = false;
  }

  return options;
}

function resolveTargetUrl(webBaseUrl, target) {
  return /^https?:\/\//u.test(target) ? target : new URL(target, webBaseUrl).toString();
}

function createRunArtifacts({ repoRoot, mode, outDir, now = new Date() }) {
  if (mode === 'login') {
    return {
      runDir: null,
      metaPath: null,
      storageStatePath: null,
      htmlPath: null,
      screenshotPath: null,
      consoleLogPath: null,
    };
  }

  const timestamp = now.toISOString().replaceAll(':', '-').replaceAll('.', '-');
  const runDir = outDir
    ? path.resolve(repoRoot, outDir)
    : path.join(repoRoot, 'tmp', 'page-debug', timestamp);

  return {
    runDir,
    metaPath: path.join(runDir, 'meta.json'),
    storageStatePath: path.join(runDir, 'storage-state.json'),
    htmlPath: mode === 'snapshot' ? path.join(runDir, 'index.html') : null,
    screenshotPath: path.join(runDir, 'page.png'),
    consoleLogPath: path.join(runDir, 'console.ndjson'),
  };
}

function createSuccessResult({
  mode,
  requestedUrl,
  finalUrl,
  authenticated,
  readyState,
  artifacts,
  warnings,
}) {
  return {
    ok: true,
    mode,
    requestedUrl,
    finalUrl,
    authenticated,
    readyState,
    outputDir: artifacts.runDir,
    metaPath: artifacts.metaPath,
    storageStatePath: artifacts.storageStatePath,
    htmlPath: artifacts.htmlPath,
    screenshotPath: artifacts.screenshotPath,
    consoleLogPath: artifacts.consoleLogPath,
    warnings,
  };
}

function loadPlaywright(repoRoot) {
  const webRequire = createRequire(path.join(repoRoot, 'web', 'package.json'));
  return webRequire('playwright');
}

function buildChromiumLaunchOptions({ headless, env = process.env }) {
  const executablePath = env[CHROMIUM_EXECUTABLE_ENV]?.trim();

  return executablePath
    ? { headless, executablePath }
    : { headless };
}

function writeStdoutJson(payload) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
}

async function captureDomSnapshot(page) {
  return page.evaluate(
    ({ stylePrefix, scriptPrefix }) => {
      const clone = document.documentElement.cloneNode(true);
      const ownerDocument = clone.ownerDocument || document;
      const inlineStyles = [];
      const inlineScripts = [];

      clone.querySelectorAll('style').forEach((node, index) => {
        const placeholder = `${stylePrefix}${index + 1}__`;
        inlineStyles.push({ placeholder, content: node.textContent || '' });
        const link = ownerDocument.createElement('link');
        link.setAttribute('rel', 'stylesheet');
        link.setAttribute('href', placeholder);
        node.replaceWith(link);
      });

      clone.querySelectorAll('script:not([src])').forEach((node, index) => {
        const placeholder = `${scriptPrefix}${index + 1}__`;
        inlineScripts.push({ placeholder, content: node.textContent || '' });
        const script = ownerDocument.createElement('script');
        script.setAttribute('src', placeholder);
        node.replaceWith(script);
      });

      return {
        html: '<!DOCTYPE html>\n' + clone.outerHTML,
        inlineStyles,
        inlineScripts,
      };
    },
    {
      stylePrefix: INLINE_STYLE_PREFIX,
      scriptPrefix: INLINE_SCRIPT_PREFIX,
    }
  );
}

function writeMetaFile(metaPath, meta) {
  fs.writeFileSync(metaPath, JSON.stringify(meta, null, 2) + '\n', 'utf8');
}

async function runPageDebug(options, deps = {}) {
  const repoRoot = deps.repoRoot || getRepoRoot();
  const playwright = deps.playwright || loadPlaywright(repoRoot);
  const resolveCredentials = deps.loadRootCredentials || loadRootCredentials;
  const emitStdout = deps.writeStdoutJson || writeStdoutJson;
  const credentials = resolveCredentials({
    repoRoot,
    accountOverride: options.account,
    passwordOverride: options.password,
  });
  const artifacts = createRunArtifacts({
    repoRoot,
    mode: options.mode,
    outDir: options.outDir,
  });
  const warnings = [];

  if (artifacts.runDir) {
    fs.mkdirSync(artifacts.runDir, { recursive: true });
  }

  if (options.mode === 'login') {
    await loginAndPersistStorageState({
      playwright,
      apiBaseUrl: options.apiBaseUrl,
      account: credentials.account,
      password: credentials.password,
      storageStatePath: null,
    });

    const result = createSuccessResult({
      mode: 'login',
      requestedUrl: null,
      finalUrl: null,
      authenticated: true,
      readyState: 'authenticated_only',
      artifacts,
      warnings,
    });
    emitStdout(result);
    return result;
  }

  await loginAndPersistStorageState({
    playwright,
    apiBaseUrl: options.apiBaseUrl,
    account: credentials.account,
    password: credentials.password,
    storageStatePath: artifacts.storageStatePath,
  });

  const browser = await playwright.chromium.launch(
    buildChromiumLaunchOptions({ headless: options.headless })
  );
  let keepBrowserOpen = options.mode === 'open';

  try {
    const context = await browser.newContext({ storageState: artifacts.storageStatePath });
    const page = await context.newPage();
    const collector = createConsoleCollector();
    const resourceRecords = [];
    const resourceTasks = new Set();

    collector.attach(page);
    page.on('response', (response) => {
      const captureTask = (async () => {
        const resourceType = response.request().resourceType();
        if (!['stylesheet', 'script'].includes(resourceType) || !response.ok()) {
          return;
        }

        try {
          resourceRecords.push({
            kind: resourceType,
            originalUrl: response.url(),
            body: await response.text(),
          });
        } catch (error) {
          warnings.push(
            `capture_response_failed:${response.url()}:${error instanceof Error ? error.message : String(error)}`
          );
        }
      })();

      resourceTasks.add(captureTask);
      captureTask.finally(() => {
        resourceTasks.delete(captureTask);
      });
    });

    const requestedUrl = resolveTargetUrl(options.webBaseUrl, options.target);
    await page.goto(requestedUrl, {
      waitUntil: 'domcontentloaded',
      timeout: options.timeout,
    });

    const ready = await waitForPageReady({
      page,
      requestedUrl: options.target,
      waitForUrl: options.waitForUrl,
      waitForSelector: options.waitForSelector,
      timeout: options.timeout,
    });

    await Promise.allSettled([...resourceTasks]);

    await writeEvidence({
      page,
      screenshotPath: artifacts.screenshotPath,
      consoleLogPath: artifacts.consoleLogPath,
      collector,
    });

    if (options.mode === 'snapshot') {
      const externalRecords = assignLocalResourcePaths(resourceRecords);
      const externalStyles = externalRecords.filter((entry) => entry.kind === 'stylesheet');
      const externalScripts = externalRecords.filter((entry) => entry.kind === 'script');
      const domSnapshot = await captureDomSnapshot(page);
      const inlineArtifacts = assignInlineArtifactPaths({
        inlineStyles: domSnapshot.inlineStyles,
        inlineScripts: domSnapshot.inlineScripts,
        externalStyles,
        externalScripts,
      });
      const html = rewriteSnapshotHtml(domSnapshot.html, {
        externalStyles,
        externalScripts,
        inlineStyles: inlineArtifacts.inlineStyles,
        inlineScripts: inlineArtifacts.inlineScripts,
      });
      const meta = buildMetaPayload({
        requestedUrl: options.target,
        finalUrl: ready.finalUrl,
        webBaseUrl: options.webBaseUrl,
        apiBaseUrl: options.apiBaseUrl,
        account: credentials.account,
        readyState: ready.readyState,
        storageStatePath: artifacts.storageStatePath,
        screenshotPath: artifacts.screenshotPath,
        consoleLogPath: artifacts.consoleLogPath,
        consoleEntries: collector.entries,
        resources: [...externalRecords, ...inlineArtifacts.inlineStyles, ...inlineArtifacts.inlineScripts],
        warnings,
      });

      writeSnapshotArtifacts({
        runDir: artifacts.runDir,
        htmlPath: artifacts.htmlPath,
        metaPath: artifacts.metaPath,
        html,
        meta,
        externalStyles,
        externalScripts,
        inlineStyles: inlineArtifacts.inlineStyles,
        inlineScripts: inlineArtifacts.inlineScripts,
      });
    } else {
      writeMetaFile(
        artifacts.metaPath,
        buildMetaPayload({
          requestedUrl: options.target,
          finalUrl: ready.finalUrl,
          webBaseUrl: options.webBaseUrl,
          apiBaseUrl: options.apiBaseUrl,
          account: credentials.account,
          readyState: ready.readyState,
          storageStatePath: artifacts.storageStatePath,
          screenshotPath: artifacts.screenshotPath,
          consoleLogPath: artifacts.consoleLogPath,
          consoleEntries: collector.entries,
          resources: [],
          warnings,
        })
      );
    }

    const result = createSuccessResult({
      mode: options.mode,
      requestedUrl: options.target,
      finalUrl: ready.finalUrl,
      authenticated: true,
      readyState: ready.readyState,
      artifacts,
      warnings,
    });

    emitStdout(result);
    return result;
  } catch (error) {
    keepBrowserOpen = false;
    throw error;
  } finally {
    if (!keepBrowserOpen) {
      await browser.close();
    }
  }
}

async function main(argv = process.argv.slice(2)) {
  const options = parseCliArgs(argv);
  if (options.help) {
    process.stdout.write('用法：node scripts/node/page-debug/cli.js [snapshot|open|login] <route-or-url>\n');
    return 0;
  }

  const result = await runPageDebug(options);
  return result.ok ? 0 : 1;
}

module.exports = {
  buildChromiumLaunchOptions,
  DEFAULT_API_BASE_URL,
  DEFAULT_TIMEOUT,
  DEFAULT_WEB_BASE_URL,
  createRunArtifacts,
  createSuccessResult,
  getRepoRoot,
  loadPlaywright,
  main,
  parseCliArgs,
  resolveTargetUrl,
  runPageDebug,
  writeStdoutJson,
};
