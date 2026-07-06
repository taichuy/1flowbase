const fs = require('node:fs');
const net = require('node:net');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { createRequire } = require('node:module');
const {
  buildNodePreferredEnv,
  resolvePnpmBinaryFromPath
} = require('../testing/node-runtime.js');

const MODES = new Set(['component', 'page', 'file', 'all-pages']);
const USER_FRONTEND_PORT = 3100;
const DEFAULT_STYLE_BOUNDARY_BASE_URL = `http://127.0.0.1:${USER_FRONTEND_PORT}`;
const DEFAULT_TEMPORARY_FRONTEND_PORT = 3101;
const TEMPORARY_FRONTEND_HOST = '127.0.0.1';
const TEMPORARY_FRONTEND_STARTUP_TIMEOUT_MS = 30_000;
const TEMPORARY_FRONTEND_POLL_INTERVAL_MS = 500;
const TEMPORARY_FRONTEND_STOP_TIMEOUT_MS = 5_000;
const STYLE_BOUNDARY_USER_PERMISSIONS = [
  'application.view.all',
  'application.edit.own',
  'application.create.all',
  'embedded_app.view.all',
  'api_reference.view.all',
  'system_runtime.view.all',
  'state_model.view.all',
  'state_model.manage.all',
  'file_table.view.all',
  'file_object.view.all',
  'file_storage.view.all',
  'frontstage.page.design',
  'user.view.all',
  'user.manage.all',
  'role_permission.view.all',
  'role_permission.manage.all'
];
const NODE_DETAIL_DOCK_SCENE_IDS = new Set([
  'component.agent-flow-node-detail',
  'page.application-detail'
]);

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function parseCliArgs(argv) {
  if (argv.includes('-h') || argv.includes('--help')) {
    return { mode: 'all-pages', target: null, help: true };
  }

  const [mode, target = null] = argv;

  if (!MODES.has(mode)) {
    throw new Error(`Unknown mode: ${mode}`);
  }

  if (mode !== 'all-pages' && !target) {
    throw new Error(`Mode ${mode} requires a target`);
  }

  return {
    mode,
    target,
    help: false
  };
}

function usage() {
  process.stdout.write(`用法：node scripts/node/check-style-boundary/cli.js <component|page|file|all-pages> [target]

示例：
  node scripts/node/check-style-boundary/cli.js component component.account-popup
  node scripts/node/check-style-boundary/cli.js page page.home
  node scripts/node/check-style-boundary/cli.js file web/app/src/styles/global.css
  node scripts/node/check-style-boundary/cli.js all-pages
`);
}

function loadManifest(repoRoot) {
  const manifestPath = path.join(
    repoRoot,
    'web',
    'app',
    'src',
    'style-boundary',
    'scenario-manifest.json'
  );

  return JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
}

function resolveSceneIds(manifest, options) {
  switch (options.mode) {
    case 'component':
    case 'page':
      return [options.target];
    case 'all-pages':
      return manifest.filter((scene) => scene.kind === 'page').map((scene) => scene.id);
    case 'file': {
      const matched = manifest
        .filter((scene) => scene.impactFiles.includes(options.target))
        .map((scene) => scene.id);

      if (matched.length === 0) {
        throw new Error(`样式扩散失败：未声明 ${options.target} 的页面/组件场景映射`);
      }

      return matched;
    }
    default:
      throw new Error(`Unsupported mode: ${options.mode}`);
  }
}

function createProbeUrl(baseUrl, sceneId) {
  return `${baseUrl}/style-boundary.html?scene=${encodeURIComponent(sceneId)}`;
}

function hasExplicitStyleBoundaryBaseUrl(env = process.env) {
  return typeof env.STYLE_BOUNDARY_BASE_URL === 'string' &&
    env.STYLE_BOUNDARY_BASE_URL.trim() !== '';
}

function resolveStyleBoundaryBaseUrl(env = process.env) {
  if (hasExplicitStyleBoundaryBaseUrl(env)) {
    return env.STYLE_BOUNDARY_BASE_URL.trim().replace(/\/+$/u, '');
  }

  return DEFAULT_STYLE_BOUNDARY_BASE_URL;
}

function parseTemporaryFrontendPort(rawPort) {
  const port = Number(rawPort);

  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error(`STYLE_BOUNDARY_PORT must be an integer between 1 and 65535: ${rawPort}`);
  }

  if (port === USER_FRONTEND_PORT) {
    throw new Error(`STYLE_BOUNDARY_PORT must not be ${USER_FRONTEND_PORT}; that port belongs to the user frontend`);
  }

  return port;
}

async function isPortAvailable(port, host = TEMPORARY_FRONTEND_HOST) {
  return new Promise((resolve) => {
    const server = net.createServer();
    let settled = false;
    const finish = (available) => {
      if (settled) {
        return;
      }

      settled = true;
      server.removeAllListeners();
      resolve(available);
    };

    server.unref();
    server.once('error', () => finish(false));
    server.listen({ host, port }, () => {
      server.close(() => finish(true));
    });
  });
}

async function findAvailableTemporaryFrontendPort(startPort, deps = {}) {
  const isCandidateAvailable = deps.isPortAvailable || isPortAvailable;

  for (let candidate = startPort; candidate <= 65535; candidate += 1) {
    if (candidate === USER_FRONTEND_PORT) {
      continue;
    }

    if (await isCandidateAvailable(candidate, TEMPORARY_FRONTEND_HOST)) {
      return candidate;
    }
  }

  throw new Error(`No available style-boundary frontend port found after ${startPort}`);
}

async function resolveTemporaryFrontendPort(env = process.env, deps = {}) {
  const configuredPort = env.STYLE_BOUNDARY_PORT;
  const isCandidateAvailable = deps.isPortAvailable || isPortAvailable;

  if (typeof configuredPort === 'string' && configuredPort.trim() !== '') {
    const port = parseTemporaryFrontendPort(configuredPort.trim());

    if (!(await isCandidateAvailable(port, TEMPORARY_FRONTEND_HOST))) {
      throw new Error(`STYLE_BOUNDARY_PORT ${port} is already in use`);
    }

    return port;
  }

  return findAvailableTemporaryFrontendPort(DEFAULT_TEMPORARY_FRONTEND_PORT, deps);
}

function buildTemporaryFrontendCommand(repoRoot, port, env = process.env) {
  const runtime = buildNodePreferredEnv(env);

  return {
    command: resolvePnpmBinaryFromPath(runtime.env) || 'pnpm',
    args: [
      'exec',
      'vite',
      '--host',
      TEMPORARY_FRONTEND_HOST,
      '--port',
      String(port),
      '--strictPort'
    ],
    cwd: path.join(repoRoot, 'web', 'app'),
    env: runtime.env
  };
}

function delay(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function formatTemporaryFrontendExit(exitState) {
  if (exitState.error) {
    return exitState.error.message;
  }

  if (exitState.signal) {
    return `signal ${exitState.signal}`;
  }

  return `exit code ${exitState.code}`;
}

async function stopTemporaryFrontendProcess(child, exitPromise) {
  if (
    child.exitCode !== null ||
    child.signalCode !== null ||
    !child.pid
  ) {
    return;
  }

  const kill = (signal) => {
    try {
      if (process.platform !== 'win32') {
        process.kill(-child.pid, signal);
        return;
      }

      child.kill(signal);
    } catch (_error) {
      try {
        child.kill(signal);
      } catch {
        // The temporary process already exited between the readiness probe and cleanup.
      }
    }
  };

  kill('SIGTERM');
  const stopped = await Promise.race([
    exitPromise,
    delay(TEMPORARY_FRONTEND_STOP_TIMEOUT_MS).then(() => null)
  ]);

  if (stopped) {
    return;
  }

  kill('SIGKILL');
  await Promise.race([
    exitPromise,
    delay(TEMPORARY_FRONTEND_STOP_TIMEOUT_MS)
  ]);
}

function startTemporaryFrontend(repoRoot, port, deps = {}) {
  const command = buildTemporaryFrontendCommand(repoRoot, port, deps.env || process.env);
  const spawnImpl = deps.spawnImpl || spawn;
  let exitState = null;
  const child = spawnImpl(command.command, command.args, {
    cwd: command.cwd,
    env: command.env,
    stdio: 'inherit',
    detached: process.platform !== 'win32'
  });
  const exitPromise = new Promise((resolve) => {
    child.once('error', (error) => {
      exitState = { error };
      resolve(exitState);
    });
    child.once('exit', (code, signal) => {
      exitState = { code, signal };
      resolve(exitState);
    });
  });

  return {
    baseUrl: `http://${TEMPORARY_FRONTEND_HOST}:${port}`,
    child,
    getExitState: () => exitState,
    stop: () => stopTemporaryFrontendProcess(child, exitPromise)
  };
}

async function waitForTemporaryFrontendReady(browser, frontend, sceneId, options = {}) {
  const timeoutMs = options.timeoutMs || TEMPORARY_FRONTEND_STARTUP_TIMEOUT_MS;
  const pollIntervalMs = options.pollIntervalMs || TEMPORARY_FRONTEND_POLL_INTERVAL_MS;
  const deadline = Date.now() + timeoutMs;

  while (Date.now() < deadline) {
    const exitState = frontend.getExitState();

    if (exitState) {
      throw new Error(
        `style-boundary temporary frontend exited before ready: ${formatTemporaryFrontendExit(exitState)}`
      );
    }

    if (await isStyleBoundaryFrontendReady(browser, frontend.baseUrl, sceneId)) {
      return;
    }

    await delay(Math.min(pollIntervalMs, Math.max(deadline - Date.now(), 0)));
  }

  throw new Error(
    `style-boundary temporary frontend was not ready at ${frontend.baseUrl} within ${timeoutMs}ms`
  );
}

async function resolveStyleBoundaryFrontendHost(
  browser,
  repoRoot,
  sceneId,
  env = process.env,
  deps = {}
) {
  const baseUrl = resolveStyleBoundaryBaseUrl(env);
  const isReady = deps.isStyleBoundaryFrontendReady || isStyleBoundaryFrontendReady;
  const resolvePort = deps.resolveTemporaryFrontendPort || resolveTemporaryFrontendPort;
  const startFrontend = deps.startTemporaryFrontend || startTemporaryFrontend;
  const waitForReady = deps.waitForTemporaryFrontendReady || waitForTemporaryFrontendReady;
  const writeStdout = deps.writeStdout || ((message) => process.stdout.write(message));

  if (await isReady(browser, baseUrl, sceneId)) {
    return {
      baseUrl,
      stop: async () => {}
    };
  }

  if (hasExplicitStyleBoundaryBaseUrl(env)) {
    throw new Error(`style-boundary frontend is not ready at STYLE_BOUNDARY_BASE_URL=${baseUrl}`);
  }

  const port = await resolvePort(env, deps);
  const frontend = startFrontend(repoRoot, port, { env });

  writeStdout(`[1flowbase-style-boundary] starting isolated frontend ${frontend.baseUrl}\n`);

  try {
    await waitForReady(browser, frontend, sceneId);
    return frontend;
  } catch (error) {
    await frontend.stop();
    throw error;
  }
}

async function isStyleBoundaryFrontendReady(browser, baseUrl, sceneId) {
  const page = await browser.newPage();

  try {
    await installStyleBoundaryNetworkMocks(page);
    await page.goto(createProbeUrl(baseUrl, sceneId), {
      waitUntil: 'domcontentloaded',
      timeout: 5000,
    });
    await page.waitForFunction(() => window.__STYLE_BOUNDARY__?.ready === true, {
      timeout: 5000,
    });
    return true;
  } catch (_error) {
    return false;
  } finally {
    await page.close();
  }
}

function isRecord(value) {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function createStyleBoundaryMeResponse(meta = {}) {
  return {
    data: {
      id: 'user-1',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'Captain Root',
      name: 'Root',
      avatar_url: null,
      introduction: 'Boundary user',
      preferred_locale: null,
      meta,
      effective_display_role: 'root',
      permissions: STYLE_BOUNDARY_USER_PERMISSIONS
    },
    meta: null
  };
}

function readStyleBoundaryMetaPatch(request) {
  try {
    const body = typeof request.postDataJSON === 'function'
      ? request.postDataJSON()
      : null;

    if (isRecord(body) && isRecord(body.meta)) {
      return body.meta;
    }
  } catch (_error) {
    return {};
  }

  return {};
}

async function installStyleBoundaryNetworkMocks(page) {
  await page.route('**/api/console/me/meta', async (route) => {
    const meta = readStyleBoundaryMetaPatch(route.request());

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(createStyleBoundaryMeResponse(meta))
    });
  });
}

function loadPlaywright(repoRoot) {
  const webRequire = createRequire(path.join(repoRoot, 'web', 'package.json'));
  return webRequire('playwright');
}

async function collectNodeResult(page, cdp, styleSheets, node) {
  const locator = page.locator(node.selector).first();
  await locator.waitFor();

  await locator.evaluate((element) => {
    element.setAttribute('data-style-boundary-probe', 'active');
  });

  const actual = await locator.evaluate((element, propertyAssertions) => {
    const styles = window.getComputedStyle(element);

    return Object.fromEntries(
      propertyAssertions.map((assertion) => [
        assertion.property,
        styles.getPropertyValue(assertion.property)
      ])
    );
  }, node.propertyAssertions);

  const { root } = await cdp.send('DOM.getDocument', {});
  const nodeId = await cdp.send('DOM.querySelector', {
    nodeId: root.nodeId,
    selector: '[data-style-boundary-probe="active"]'
  });
  const matched = await cdp.send('CSS.getMatchedStylesForNode', { nodeId: nodeId.nodeId });

  await locator.evaluate((element) => {
    element.removeAttribute('data-style-boundary-probe');
  });

  return {
    node,
    actual,
    matchedRules: (matched.matchedCSSRules || []).map((ruleMatch) => ({
      selector: ruleMatch.rule.selectorList.text,
      origin: ruleMatch.rule.origin,
      sourceUrl: styleSheets.get(ruleMatch.rule.style.styleSheetId) || 'inline'
    }))
  };
}

function collectViolations(results) {
  return results.flatMap((result) =>
    result.node.propertyAssertions
      .filter((assertion) => result.actual[assertion.property] !== assertion.expected)
      .map((assertion) => ({
        nodeId: result.node.id,
        selector: result.node.selector,
        property: assertion.property,
        expected: assertion.expected,
        actual: result.actual[assertion.property],
        matchedRules: result.matchedRules
      }))
  );
}

function createRectIntersection(subjectRect, referenceRect) {
  const left = Math.max(subjectRect.left, referenceRect.left);
  const right = Math.min(subjectRect.right, referenceRect.right);
  const top = Math.max(subjectRect.top, referenceRect.top);
  const bottom = Math.min(subjectRect.bottom, referenceRect.bottom);

  if (right <= left || bottom <= top) {
    return null;
  }

  return {
    left,
    right,
    top,
    bottom,
    width: right - left,
    height: bottom - top,
  };
}

function getMissingMeasurementViolation(assertion, missingField) {
  return {
    assertionId: assertion.id,
    type: assertion.type,
    actual: 'missing_element',
    details: `${missingField}_missing`,
    subjectSelector: assertion.subjectSelector,
    referenceSelector: assertion.referenceSelector || null,
    containerSelector: assertion.containerSelector || null,
  };
}

function collectRelationshipViolations(assertions = [], measurements = {}) {
  return assertions.flatMap((assertion) => {
    const subject = measurements[assertion.subjectSelector];
    const subjectRect = subject?.rect;

    if (!subject?.exists || !subjectRect) {
      return [getMissingMeasurementViolation(assertion, 'subject')];
    }

    if (assertion.type === 'no_overlap') {
      const reference = measurements[assertion.referenceSelector];
      const referenceRect = reference?.rect;

      if (!reference?.exists || !referenceRect) {
        return [getMissingMeasurementViolation(assertion, 'reference')];
      }

      const intersection = createRectIntersection(subjectRect, referenceRect);

      if (!intersection) {
        return [];
      }

      return [
        {
          assertionId: assertion.id,
          type: assertion.type,
          actual: 'overlap',
          details: `intersection=${intersection.width}x${intersection.height}`,
          subjectSelector: assertion.subjectSelector,
          referenceSelector: assertion.referenceSelector,
          containerSelector: null,
        },
      ];
    }

    if (assertion.type === 'within_container') {
      const container = measurements[assertion.containerSelector];
      const containerRect = container?.rect;

      if (!container?.exists || !containerRect) {
        return [getMissingMeasurementViolation(assertion, 'container')];
      }

      const overflow = {
        left: Math.max(0, containerRect.left - subjectRect.left),
        right: Math.max(0, subjectRect.right - containerRect.right),
        top: Math.max(0, containerRect.top - subjectRect.top),
        bottom: Math.max(0, subjectRect.bottom - containerRect.bottom),
      };

      if (overflow.left === 0 && overflow.right === 0 && overflow.top === 0 && overflow.bottom === 0) {
        return [];
      }

      return [
        {
          assertionId: assertion.id,
          type: assertion.type,
          actual: 'outside_container',
          details: `overflow=left:${overflow.left},right:${overflow.right},top:${overflow.top},bottom:${overflow.bottom}`,
          subjectSelector: assertion.subjectSelector,
          referenceSelector: null,
          containerSelector: assertion.containerSelector,
        },
      ];
    }

    if (assertion.type === 'min_gap') {
      const reference = measurements[assertion.referenceSelector];
      const referenceRect = reference?.rect;

      if (!reference?.exists || !referenceRect) {
        return [getMissingMeasurementViolation(assertion, 'reference')];
      }

      const axis = assertion.axis || 'horizontal';
      const gap = axis === 'vertical'
        ? referenceRect.top - subjectRect.bottom
        : referenceRect.left - subjectRect.right;

      if (gap >= assertion.minGap) {
        return [];
      }

      return [
        {
          assertionId: assertion.id,
          type: assertion.type,
          actual: 'gap_too_small',
          details: `axis=${axis} expected>=${assertion.minGap} actual=${gap}`,
          subjectSelector: assertion.subjectSelector,
          referenceSelector: assertion.referenceSelector,
          containerSelector: null,
        },
      ];
    }

    if (assertion.type === 'fully_visible') {
      if (subject.withinViewport === false) {
        return [
          {
            assertionId: assertion.id,
            type: assertion.type,
            actual: 'outside_viewport',
            details: 'subject_outside_viewport',
            subjectSelector: assertion.subjectSelector,
            referenceSelector: null,
            containerSelector: null,
          },
        ];
      }

      if ((subject.visibleSamples || []).every((sample) => sample === true)) {
        return [];
      }

      return [
        {
          assertionId: assertion.id,
          type: assertion.type,
          actual: 'partially_occluded',
          details: `visible_samples=${JSON.stringify(subject.visibleSamples || [])}`,
          subjectSelector: assertion.subjectSelector,
          referenceSelector: null,
          containerSelector: null,
        },
      ];
    }

    throw new Error(`Unknown relationship assertion type: ${assertion.type}`);
  });
}

function formatBoundaryFailure(sceneId, violations) {
  return `样式边界失败：${sceneId} ${violations
    .map(
      (violation) =>
        `${violation.nodeId}.${violation.property} expected=${violation.expected} actual=${violation.actual} source=${violation.matchedRules
          .map((rule) => `${rule.sourceUrl}::${rule.selector}`)
          .join('|')}`
    )
    .join(', ')}`;
}

function formatRelationshipFailure(sceneId, violations) {
  return `布局关系失败：${sceneId} ${violations
    .map((violation) => {
      const segments = [
        `${violation.assertionId}.${violation.type}`,
        `actual=${violation.actual}`,
        `subject=${violation.subjectSelector}`,
      ];

      if (violation.referenceSelector) {
        segments.push(`reference=${violation.referenceSelector}`);
      }

      if (violation.containerSelector) {
        segments.push(`container=${violation.containerSelector}`);
      }

      if (violation.details) {
        segments.push(`details=${violation.details}`);
      }

      return segments.join(' ');
    })
    .join(', ')}`;
}

async function collectRelationshipMeasurements(page, assertions = []) {
  if (assertions.length === 0) {
    return {};
  }

  return page.evaluate((sceneAssertions) => {
    const selectors = new Set();
    const fullyVisibleSelectors = new Set();

    for (const assertion of sceneAssertions) {
      selectors.add(assertion.subjectSelector);

      if (assertion.referenceSelector) {
        selectors.add(assertion.referenceSelector);
      }

      if (assertion.containerSelector) {
        selectors.add(assertion.containerSelector);
      }

      if (assertion.type === 'fully_visible') {
        fullyVisibleSelectors.add(assertion.subjectSelector);
      }
    }

    const clampSamplePoint = (value, min, max) => {
      if (max <= min) {
        return min;
      }

      return Math.min(max, Math.max(min, value));
    };

    const createSamplePoints = (rect) => {
      const insetX = Math.min(4, Math.max(1, rect.width / 4));
      const insetY = Math.min(4, Math.max(1, rect.height / 4));
      const minX = rect.left;
      const maxX = Math.max(rect.left, rect.right - 1);
      const minY = rect.top;
      const maxY = Math.max(rect.top, rect.bottom - 1);

      return [
        { x: clampSamplePoint(rect.left + insetX, minX, maxX), y: clampSamplePoint(rect.top + insetY, minY, maxY) },
        { x: clampSamplePoint(rect.right - insetX, minX, maxX), y: clampSamplePoint(rect.top + insetY, minY, maxY) },
        { x: clampSamplePoint(rect.left + insetX, minX, maxX), y: clampSamplePoint(rect.bottom - insetY, minY, maxY) },
        { x: clampSamplePoint(rect.right - insetX, minX, maxX), y: clampSamplePoint(rect.bottom - insetY, minY, maxY) },
        { x: clampSamplePoint(rect.left + rect.width / 2, minX, maxX), y: clampSamplePoint(rect.top + rect.height / 2, minY, maxY) },
      ];
    };

    for (const selector of fullyVisibleSelectors) {
      const element = document.querySelector(selector);

      if (element) {
        element.scrollIntoView({
          block: 'center',
          inline: 'center',
        });
      }
    }

    return Object.fromEntries(
      [...selectors].map((selector) => {
        const element = document.querySelector(selector);

        if (!element) {
          return [selector, { exists: false, rect: null, withinViewport: false, visibleSamples: [] }];
        }

        const bounds = element.getBoundingClientRect();
        const rect = {
          left: bounds.left,
          top: bounds.top,
          right: bounds.right,
          bottom: bounds.bottom,
          width: bounds.width,
          height: bounds.height,
        };
        const withinViewport =
          rect.left >= 0 &&
          rect.top >= 0 &&
          rect.right <= window.innerWidth &&
          rect.bottom <= window.innerHeight;

        const visibleSamples = fullyVisibleSelectors.has(selector)
          ? createSamplePoints(rect).map((point) => {
              const hit = document.elementFromPoint(point.x, point.y);
              return Boolean(hit && (hit === element || element.contains(hit)));
            })
          : [];

        return [
          selector,
          {
            exists: true,
            rect,
            withinViewport,
            visibleSamples,
          },
        ];
      })
    );
  }, assertions);
}

async function prepareSceneForAssertions(page, scene) {
  if (!NODE_DETAIL_DOCK_SCENE_IDS.has(scene.id)) {
    return;
  }

  if (await isNodeDetailDockVisible(page)) {
    return;
  }

  const opened = await openNodeDetailDock(page);

  if (!opened) {
    throw new Error(
      `style-boundary ${scene.id} failed to open the node detail dock`
    );
  }
}

async function isNodeDetailDockVisible(page) {
  const detailDock = page.locator('.agent-flow-editor__detail-dock').first();
  return (
    (await detailDock.count()) > 0 &&
    await detailDock.isVisible().catch(() => false)
  );
}

async function openNodeDetailDock(page) {
  const nodeSelector = '.agent-flow-node-card--type-llm';
  const dockSelector = '.agent-flow-editor__detail-dock';

  await page.waitForSelector(nodeSelector, {
    state: 'visible',
    timeout: 30000,
  });

  for (let attempt = 0; attempt < 5; attempt += 1) {
    const clicked = await page.evaluate(({ nodeSelector, dockSelector }) => {
      const existingDock = document.querySelector(dockSelector);

      if (existingDock && existingDock.getBoundingClientRect().width > 0) {
        return true;
      }

      const node = document.querySelector(nodeSelector);

      if (!node) {
        return false;
      }

      node.scrollIntoView({
        block: 'center',
        inline: 'center',
      });
      node.dispatchEvent(new MouseEvent('click', {
        bubbles: true,
        cancelable: true,
        view: window,
      }));
      return true;
    }, { nodeSelector, dockSelector });

    if (!clicked) {
      await page.waitForTimeout(100);
      continue;
    }

    if (await isNodeDetailDockVisible(page)) {
      return true;
    }

    await page.waitForTimeout(250);
  }

  return isNodeDetailDockVisible(page);
}

async function runScene(browser, baseUrl, scene) {
  const page = await browser.newPage();
  await installStyleBoundaryNetworkMocks(page);
  const cdp = await page.context().newCDPSession(page);
  const styleSheets = new Map();

  await cdp.send('DOM.enable');
  await cdp.send('CSS.enable');
  cdp.on('CSS.styleSheetAdded', (event) => {
    styleSheets.set(event.header.styleSheetId, event.header.sourceURL || 'inline');
  });

  await page.goto(createProbeUrl(baseUrl, scene.id), {
    waitUntil: 'domcontentloaded'
  });
  await page.waitForFunction(() => window.__STYLE_BOUNDARY__?.ready === true);
  await prepareSceneForAssertions(page, scene);

  const nodeResults = [];

  for (const node of scene.boundaryNodes) {
    nodeResults.push(await collectNodeResult(page, cdp, styleSheets, node));
  }
  const relationshipMeasurements = await collectRelationshipMeasurements(
    page,
    scene.relationshipAssertions || []
  );

  return {
    page,
    scene,
    violations: collectViolations(nodeResults),
    relationshipViolations: collectRelationshipViolations(
      scene.relationshipAssertions || [],
      relationshipMeasurements
    )
  };
}

function ensureUploadsDir(repoRoot) {
  const uploadsDir = path.join(repoRoot, 'uploads', 'style-boundary');
  fs.mkdirSync(uploadsDir, { recursive: true });
  return uploadsDir;
}

async function main(argv) {
  const options = parseCliArgs(argv);

  if (options.help) {
    usage();
    return;
  }

  const repoRoot = getRepoRoot();
  const manifest = loadManifest(repoRoot);
  const sceneIds = resolveSceneIds(manifest, options);
  const uploadsDir = ensureUploadsDir(repoRoot);

  const { chromium } = loadPlaywright(repoRoot);
  const browser = await chromium.launch({
    channel: 'chrome',
    headless: true
  });
  let frontendHost = {
    baseUrl: resolveStyleBoundaryBaseUrl(process.env),
    stop: async () => {}
  };

  try {
    frontendHost = await resolveStyleBoundaryFrontendHost(
      browser,
      repoRoot,
      sceneIds[0],
      process.env
    );

    for (const sceneId of sceneIds) {
      const scene = manifest.find((entry) => entry.id === sceneId);

      if (!scene) {
        throw new Error(`Unknown style boundary scene: ${sceneId}`);
      }

      const result = await runScene(browser, frontendHost.baseUrl, scene);

      if (
        result.violations.length > 0 ||
        result.relationshipViolations.length > 0
      ) {
        const screenshotPath = path.join(uploadsDir, `${scene.id}.png`);
        await result.page.screenshot({ path: screenshotPath, fullPage: true });
        const failureMessages = [];

        if (result.violations.length > 0) {
          failureMessages.push(formatBoundaryFailure(scene.id, result.violations));
        }

        if (result.relationshipViolations.length > 0) {
          failureMessages.push(
            formatRelationshipFailure(scene.id, result.relationshipViolations)
          );
        }

        throw new Error(failureMessages.join('\n'));
      }

      process.stdout.write(`[1flowbase-style-boundary] PASS ${scene.id}\n`);
      await result.page.close();
    }
  } finally {
    await frontendHost.stop();
    await browser.close();
  }
}

module.exports = {
  buildTemporaryFrontendCommand,
  collectRelationshipViolations,
  createProbeUrl,
  formatBoundaryFailure,
  formatRelationshipFailure,
  installStyleBoundaryNetworkMocks,
  isStyleBoundaryFrontendReady,
  main,
  parseCliArgs,
  resolveStyleBoundaryBaseUrl,
  resolveStyleBoundaryFrontendHost,
  resolveTemporaryFrontendPort,
  resolveSceneIds
};
