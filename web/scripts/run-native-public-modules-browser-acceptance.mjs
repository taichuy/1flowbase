import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = resolve(webRoot, '..');
const output = resolve(
  repositoryRoot,
  'tmp/test-governance/root-1466-r4-browser'
);
const base = process.env.NATIVE_FIXTURE_BASE_URL ?? 'http://127.0.0.1:4175';
const executablePath =
  process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH ?? '/usr/bin/google-chrome';

await mkdir(output, { recursive: true });
const browser = await chromium.launch({ executablePath, headless: true });
const results = [];

try {
  for (const fixture of [
    { name: 'desktop', viewport: { width: 1440, height: 1000 } },
    { name: 'mobile-390', viewport: { width: 390, height: 844 } }
  ]) {
    results.push(await verifyFixture(browser, fixture));
  }
} finally {
  await browser.close();
}

const evidence = { ok: true, base, executablePath, results };
await writeFile(
  resolve(output, 'evidence.json'),
  `${JSON.stringify(evidence, null, 2)}\n`,
  'utf8'
);
console.log(JSON.stringify(evidence, null, 2));

async function verifyFixture(browserInstance, fixture) {
  const context = await browserInstance.newContext({
    viewport: fixture.viewport
  });
  const page = await context.newPage();
  const consoleErrors = [];
  const externalRequests = [];
  const httpErrors = [];
  const failedRequests = [];
  page.on('console', (message) => {
    if (message.type() === 'error') {
      consoleErrors.push({
        text: message.text(),
        location: message.location()
      });
    }
  });
  page.on('request', (request) => {
    if (new URL(request.url()).origin !== base) {
      externalRequests.push(request.url());
    }
  });
  page.on('response', (response) => {
    if (response.status() >= 400) {
      httpErrors.push({ url: response.url(), status: response.status() });
    }
  });
  page.on('requestfailed', (request) => {
    failedRequests.push({
      url: request.url(),
      errorText: request.failure()?.errorText ?? 'unknown'
    });
  });

  try {
    await page.goto(`${base}/native-react-trial-fixture.html`, {
      waitUntil: 'networkidle'
    });
    await page.locator('[data-testid=public-modules-a]').waitFor();
    await page.locator('[data-testid=public-modules-b]').waitFor();
    await page.locator('[aria-label=editor-a] .vditor-content').waitFor();
    await page.locator('[aria-label=editor-b] .vditor-content').waitFor();
    const nativeEvidence = await page.evaluate(() => {
      const hosts = [
        ...document.querySelectorAll(
          '[data-flowbase-native-trusted-block-root]'
        )
      ];
      const publicHosts = hosts.filter((host) =>
        host.shadowRoot?.querySelector('[data-testid^=public-modules-]')
      );
      return {
        hostCount: hosts.length,
        publicHostCount: publicHosts.length,
        canvases: publicHosts.map(
          (host) => host.shadowRoot?.querySelectorAll('canvas').length ?? 0
        ),
        richStyles: publicHosts.map(
          (host) =>
            host.shadowRoot?.querySelectorAll(
              'style[data-module-source="@1flowbase/rich-text"]'
            ).length ?? 0
        ),
        nativeStyles: publicHosts.map(
          (host) =>
            host.shadowRoot?.querySelectorAll(
              'style[data-module-source="@1flowbase/native-components"]'
            ).length ?? 0
        ),
        leakedModuleStyles: document.head.querySelectorAll(
          'style[data-module-source]'
        ).length,
        supportMarkers: document.head.querySelectorAll(
          '#vditorLuteScript, #vditorIconScript'
        ).length
      };
    });
    assertNativeEvidence(fixture.name, nativeEvidence);

    const stats = page.locator('[data-testid=native-frontstage-stats]');
    const pageCanvasRendersBefore = Number(
      await stats.getAttribute('data-page-canvas-renders')
    );
    await page.getByRole('button', { name: 'input update' }).click();
    await page
      .locator('[data-testid=native-fixture-first-output]')
      .filter({ hasText: 'source-1:1' })
      .waitFor();
    const publishCompleted = await stats.getAttribute('data-publish-completed');
    const pageCanvasRendersAfter = Number(
      await stats.getAttribute('data-page-canvas-renders')
    );
    if (
      publishCompleted !== '1' ||
      pageCanvasRendersAfter !== pageCanvasRendersBefore
    ) {
      throw new Error(
        `${fixture.name} Signal evidence failed: ${JSON.stringify({
          publishCompleted,
          pageCanvasRendersBefore,
          pageCanvasRendersAfter
        })}`
      );
    }
    await page.screenshot({
      path: resolve(output, `native-${fixture.name}.png`),
      fullPage: true
    });
    await page.getByRole('button', { name: 'exit page' }).click();
    await waitForPortalCleanup(page);
    const nativeCleanup = await readCleanup(page);
    assertCleanup(fixture.name, nativeCleanup);
    assertNetworkAndConsole(
      fixture.name,
      externalRequests,
      httpErrors,
      failedRequests,
      consoleErrors
    );

    await page.goto(`${base}/public-auth-native-fixture.html`, {
      waitUntil: 'networkidle'
    });
    await page.locator('[data-testid=public-auth-native-content]').waitFor();
    await page.getByRole('button', { name: 'local state 0' }).click();
    await page.getByRole('button', { name: 'local state 1' }).waitFor();
    await page.getByRole('button', { name: 'Create an account' }).click();
    await page.getByRole('heading', { name: 'Create an account' }).waitFor();
    await page.screenshot({
      path: resolve(output, `auth-${fixture.name}.png`),
      fullPage: true
    });
    await page.getByRole('button', { name: 'exit auth page' }).click();
    await waitForPortalCleanup(page);
    const authCleanup = await readCleanup(page);
    assertCleanup(`${fixture.name} auth`, authCleanup);
    assertNetworkAndConsole(
      fixture.name,
      externalRequests,
      httpErrors,
      failedRequests,
      consoleErrors
    );

    return {
      fixture: fixture.name,
      nativeEvidence,
      nativeCleanup,
      authCleanup,
      pageCanvasRendersBefore,
      pageCanvasRendersAfter,
      publishCompleted,
      externalRequests: externalRequests.length,
      httpErrors,
      failedRequests,
      consoleErrors
    };
  } finally {
    await context.close();
  }
}

function assertNativeEvidence(name, evidence) {
  if (
    evidence.publicHostCount !== 2 ||
    evidence.canvases.some((count) => count < 1) ||
    evidence.richStyles.some((count) => count !== 1) ||
    evidence.nativeStyles.some((count) => count !== 1) ||
    evidence.leakedModuleStyles !== 0
  ) {
    throw new Error(
      `${name} ShadowRoot evidence failed: ${JSON.stringify(evidence)}`
    );
  }
}

async function waitForPortalCleanup(page) {
  await page.waitForFunction(
    () =>
      document.querySelectorAll('[data-flowbase-native-trusted-block-root]')
        .length === 0
  );
}

async function readCleanup(page) {
  return page.evaluate(() => ({
    markers: document.head.querySelectorAll(
      '#vditorLuteScript, #vditorIconScript'
    ).length,
    roots: document.querySelectorAll(
      '[data-flowbase-native-trusted-block-root]'
    ).length
  }));
}

function assertCleanup(name, cleanup) {
  if (cleanup.markers !== 0 || cleanup.roots !== 0) {
    throw new Error(`${name} cleanup failed: ${JSON.stringify(cleanup)}`);
  }
}

function assertNetworkAndConsole(
  name,
  externalRequests,
  httpErrors,
  failedRequests,
  consoleErrors
) {
  if (externalRequests.length > 0) {
    throw new Error(
      `${name} external requests: ${externalRequests.join(', ')}`
    );
  }
  if (httpErrors.length > 0) {
    throw new Error(
      `${name} HTTP errors: ${httpErrors
        .map(({ status, url }) => `${status} ${url}`)
        .join(' | ')}`
    );
  }
  if (failedRequests.length > 0) {
    throw new Error(
      `${name} failed requests: ${failedRequests
        .map(({ errorText, url }) => `${errorText} ${url}`)
        .join(' | ')}`
    );
  }
  if (consoleErrors.length > 0) {
    throw new Error(
      `${name} console errors: ${consoleErrors
        .map(({ text, location }) =>
          `${text} @ ${location.url || '<unknown>'}:${location.lineNumber}:${location.columnNumber}`
        )
        .join(' | ')}`
    );
  }
}
