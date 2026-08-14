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
    await page
      .locator('[data-testid=catalog-rich-text-probe][data-status=ready]')
      .waitFor();
    await page
      .locator('[aria-label=catalog-rich-text-editor] .vditor-content')
      .waitFor();
    await page
      .locator('[data-testid=catalog-icons-probe][data-status=ready]')
      .waitFor();
    await page.locator('[aria-label=catalog-check-circle-icon] svg').waitFor();
    const nativeEvidence = await page.evaluate(() => {
      const hosts = [
        ...document.querySelectorAll(
          '[data-flowbase-native-trusted-block-root]'
        )
      ];
      const publicHosts = hosts.filter((host) =>
        host.shadowRoot?.querySelector('[data-testid^=public-modules-]')
      );
      const richTextRoots = [
        document,
        ...hosts.map((host) => host.shadowRoot).filter(Boolean)
      ].filter((root) => root.querySelector('.vditor'));
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
        executableStyles: publicHosts.map(
          (host) =>
            host.shadowRoot?.querySelectorAll(
              'style[data-module-source="frontstage/executable-style"]'
            ).length ?? 0
        ),
        executableStyleSafety: publicHosts.map((host) => {
          const css = [
            ...(host.shadowRoot?.querySelectorAll(
              'style[data-module-source="frontstage/executable-style"]'
            ) ?? [])
          ]
            .map((style) => style.textContent ?? '')
            .join('\n');
          return {
            hasPreflight:
              /@layer base|(?:^|\})\s*(?:button|input|h[1-6])(?:,|\{)/u.test(
                css
              ),
            hasAntSelector: /\.ant-/u.test(css),
            hasArbitraryGrid: css.includes('180px 1fr'),
            hasResponsiveVariant: css.includes('@media'),
            hasHoverVariant: css.includes(':hover')
          };
        }),
        publicLayout: publicHosts.map((host) => {
          const element = host.shadowRoot?.querySelector(
            '[data-testid^=public-modules-]'
          );
          if (!element) return null;
          const style = getComputedStyle(element);
          return {
            display: style.display,
            gap: style.gap,
            padding: style.padding,
            backgroundColor: style.backgroundColor
          };
        }),
        hostLayout: (() => {
          const element = document.querySelector(
            '[data-testid=tailwind-host-fixture-probe]'
          );
          if (!element) return null;
          const style = getComputedStyle(element);
          return {
            display: style.display,
            gap: style.gap,
            padding: style.padding
          };
        })(),
        leakedModuleStyles: document.head.querySelectorAll(
          'style[data-module-source]'
        ).length,
        supportMarkers: document.head.querySelectorAll(
          '#vditorLuteScript, #vditorIconScript'
        ).length,
        richTextIcons: richTextRoots.map((root) => {
          const toolbarIcons = [
            ...root.querySelectorAll('.vditor-toolbar use')
          ];
          return {
            spriteCount: root.querySelectorAll('[data-1flowbase-vditor-icons]')
              .length,
            symbolCount: root.querySelectorAll('symbol[id^="vditor-icon-"]')
              .length,
            toolbarIconCount: toolbarIcons.length,
            unresolvedToolbarIcons: toolbarIcons.filter((icon) => {
              const href =
                icon.getAttribute('href') ?? icon.getAttribute('xlink:href');
              return (
                !href?.startsWith('#') || !root.getElementById(href.slice(1))
              );
            }).length,
            hiddenToolbarIcons: toolbarIcons.filter((icon) => {
              const rect = icon.closest('svg')?.getBoundingClientRect();
              return !rect || rect.width === 0 || rect.height === 0;
            }).length
          };
        })
      };
    });
    assertNativeEvidence(fixture.name, nativeEvidence);

    const isolatedEvidence = await waitForIsolatedReady(page, 0);
    assertIsolatedEvidence(fixture.name, isolatedEvidence);
    await page.getByRole('button', { name: 'hidden page' }).click();
    await waitForIsolatedIframeCount(page, 0);
    const isolatedHiddenCleanup = await assertIsolatedCountersStopped(
      page,
      `${fixture.name} hidden`
    );
    await page.getByRole('button', { name: 'hidden page' }).click();
    const isolatedRestoredEvidence = await waitForIsolatedReady(
      page,
      isolatedHiddenCleanup.after.messages
    );
    assertIsolatedEvidence(
      `${fixture.name} restored`,
      isolatedRestoredEvidence
    );

    const stats = page.locator('[data-testid=native-frontstage-stats]');
    await page.waitForFunction(async () => {
      const element = document.querySelector(
        '[data-testid=native-frontstage-stats]'
      );
      const before = element?.getAttribute('data-page-canvas-renders');
      await new Promise((resolvePromise) => setTimeout(resolvePromise, 250));
      return (
        before !== null &&
        before === element?.getAttribute('data-page-canvas-renders')
      );
    });
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
    const isolatedMessagesBeforeExit = Number(
      await stats.getAttribute('data-isolated-messages')
    );
    await page.getByRole('button', { name: 'exit page' }).click();
    await waitForPortalCleanup(page);
    await waitForIsolatedIframeCount(page, 0);
    const isolatedExitCleanup = await assertIsolatedCountersStopped(
      page,
      `${fixture.name} exit`,
      isolatedMessagesBeforeExit
    );
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
    await assertNoEditorDebugSurface(page, fixture.name);
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
      isolatedEvidence,
      isolatedHiddenCleanup,
      isolatedRestoredEvidence,
      isolatedExitCleanup,
      nativeCleanup,
      authCleanup,
      authEditorDebugSurface: false,
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

async function waitForIsolatedReady(page, minimumMessages) {
  await page.waitForFunction(
    ({ minimum }) => {
      const stats = document.querySelector(
        '[data-testid=native-frontstage-stats]'
      );
      const root = document.querySelector(
        '[data-testid=frontstage-isolated-block-root-isolated]'
      );
      const messages = Number(stats?.getAttribute('data-isolated-messages'));
      return (
        root?.querySelectorAll('iframe').length === 1 &&
        stats?.getAttribute('data-isolated-ready-signal') === 'settled' &&
        messages > minimum
      );
    },
    { minimum: minimumMessages }
  );
  return readIsolatedEvidence(page);
}

async function readIsolatedEvidence(page) {
  return page.evaluate(() => {
    const stats = document.querySelector(
      '[data-testid=native-frontstage-stats]'
    );
    const root = document.querySelector(
      '[data-testid=frontstage-isolated-block-root-isolated]'
    );
    const iframe = root?.querySelector('iframe');
    return {
      iframeCount: root?.querySelectorAll('iframe').length ?? 0,
      sandbox: iframe?.getAttribute('sandbox') ?? null,
      allowsSameOrigin: iframe?.sandbox.contains('allow-same-origin') ?? null,
      readySignal: stats?.getAttribute('data-isolated-ready-signal') ?? null,
      messages: Number(stats?.getAttribute('data-isolated-messages')),
      lastTick: Number(stats?.getAttribute('data-isolated-last-tick'))
    };
  });
}

function assertIsolatedEvidence(name, evidence) {
  if (
    evidence.iframeCount !== 1 ||
    evidence.sandbox !== 'allow-scripts' ||
    evidence.allowsSameOrigin !== false ||
    evidence.readySignal !== 'settled' ||
    !Number.isFinite(evidence.messages) ||
    evidence.messages < 1 ||
    !Number.isFinite(evidence.lastTick) ||
    evidence.lastTick < 1
  ) {
    throw new Error(
      `${name} isolated iframe evidence failed: ${JSON.stringify(evidence)}`
    );
  }
}

async function waitForIsolatedIframeCount(page, expectedCount) {
  await page.waitForFunction(
    ({ expected }) =>
      document.querySelectorAll(
        '[data-testid=frontstage-isolated-block-root-isolated] iframe'
      ).length === expected,
    { expected: expectedCount }
  );
}

async function assertIsolatedCountersStopped(page, name, minimumMessages = 0) {
  await page.waitForTimeout(100);
  const before = await readIsolatedCounters(page);
  await page.waitForTimeout(150);
  const after = await readIsolatedCounters(page);
  if (
    before.messages < minimumMessages ||
    after.messages !== before.messages ||
    after.lastTick !== before.lastTick
  ) {
    throw new Error(
      `${name} isolated cleanup failed: ${JSON.stringify({ before, after })}`
    );
  }
  return { iframeCount: 0, before, after };
}

async function readIsolatedCounters(page) {
  return page.evaluate(() => {
    const stats = document.querySelector(
      '[data-testid=native-frontstage-stats]'
    );
    return {
      messages: Number(stats?.getAttribute('data-isolated-messages')),
      lastTick: Number(stats?.getAttribute('data-isolated-last-tick'))
    };
  });
}

async function assertNoEditorDebugSurface(page, name) {
  const selectors = [
    '[data-testid=js-block-preview-console]',
    '[data-testid=js-block-console-pane]',
    '[data-testid=js-block-console-prompt]',
    '[role=separator]'
  ];
  for (const selector of selectors) {
    const count = await page.locator(selector).count();
    if (count !== 0) {
      throw new Error(
        `${name} Public Auth leaked editor debug UI ${selector}: ${count}`
      );
    }
  }
}

function assertNativeEvidence(name, evidence) {
  if (
    evidence.publicHostCount !== 2 ||
    evidence.canvases.some((count) => count < 1) ||
    evidence.richStyles.some((count) => count !== 1) ||
    evidence.nativeStyles.some((count) => count !== 1) ||
    evidence.executableStyles[0] !== 1 ||
    evidence.executableStyles[1] !== 0 ||
    evidence.executableStyleSafety[0]?.hasPreflight !== false ||
    evidence.executableStyleSafety[0]?.hasAntSelector !== false ||
    evidence.executableStyleSafety[0]?.hasArbitraryGrid !== true ||
    evidence.executableStyleSafety[0]?.hasResponsiveVariant !== true ||
    evidence.executableStyleSafety[0]?.hasHoverVariant !== true ||
    evidence.publicLayout[0]?.display !== 'grid' ||
    evidence.publicLayout[0]?.gap !== '16px' ||
    evidence.publicLayout[0]?.padding !== '16px' ||
    evidence.publicLayout[0]?.backgroundColor !== 'rgb(0, 171, 115)' ||
    evidence.publicLayout[1]?.display !== 'block' ||
    evidence.publicLayout[1]?.gap !== 'normal' ||
    evidence.publicLayout[1]?.padding !== '0px' ||
    evidence.hostLayout?.display !== 'block' ||
    evidence.hostLayout?.gap !== 'normal' ||
    evidence.hostLayout?.padding !== '0px' ||
    evidence.leakedModuleStyles !== 0 ||
    evidence.richTextIcons.length !== 3 ||
    evidence.richTextIcons.some(
      (icons) =>
        icons.spriteCount !== 1 ||
        icons.symbolCount === 0 ||
        icons.toolbarIconCount === 0 ||
        icons.unresolvedToolbarIcons !== 0 ||
        icons.hiddenToolbarIcons !== 0
    )
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
        .map(
          ({ text, location }) =>
            `${text} @ ${location.url || '<unknown>'}:${location.lineNumber}:${location.columnNumber}`
        )
        .join(' | ')}`
    );
  }
}
