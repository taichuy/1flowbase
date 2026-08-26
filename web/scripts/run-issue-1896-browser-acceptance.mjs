import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = resolve(webRoot, '..');
const output = resolve(
  repositoryRoot,
  'tmp/test-governance/issue-1896-browser'
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
  const pageErrors = [];
  const httpErrors = [];
  const failedRequests = [];

  page.on('console', (message) => {
    if (message.type() === 'error') consoleErrors.push(message.text());
  });
  page.on('pageerror', (error) => pageErrors.push(error.message));
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
    const stats = page.locator('[data-testid=native-frontstage-stats]');
    const firstOutput = page.locator(
      '[data-testid=native-fixture-first-output]'
    );
    await firstOutput.waitFor({ state: 'attached' });
    await page
      .locator('[data-testid=block-slot-public-a]')
      .waitFor({ state: 'attached' });

    const initial = await readLifecycle(stats, firstOutput);
    const transitions = [];
    for (const priority of [2, 3, 1]) {
      await page.getByRole('button', { name: `demand ${priority}` }).click();
      await page.waitForFunction(
        ([expected]) =>
          document
            .querySelector('[data-testid=native-frontstage-stats]')
            ?.getAttribute('data-demands')
            ?.startsWith(`${expected},`) ?? false,
        [priority]
      );
      const current = await readLifecycle(stats, firstOutput);
      assertEqual(current.mounts, initial.mounts, `${fixture.name} mounts`);
      assertEqual(
        current.unmounts,
        initial.unmounts,
        `${fixture.name} unmounts`
      );
      assertEqual(
        current.hookIdentity,
        initial.hookIdentity,
        `${fixture.name} hook identity`
      );
      transitions.push({ priority, ...current });
    }

    const containment = await page
      .locator('[data-testid=block-slot-first]')
      .evaluate((element) => {
        const style = getComputedStyle(element);
        return {
          contentVisibility: style.contentVisibility,
          containIntrinsicSize: style.containIntrinsicSize,
          intrinsicHeight: Number(
            element.getAttribute('data-flowbase-frontstage-intrinsic-height')
          )
        };
      });
    assertEqual(
      containment.contentVisibility,
      'auto',
      `${fixture.name} content-visibility`
    );
    if (
      containment.intrinsicHeight <= 0 ||
      !containment.containIntrinsicSize.includes('px')
    ) {
      throw new Error(
        `${fixture.name} intrinsic containment collapsed: ${JSON.stringify(containment)}`
      );
    }

    const anchoring = await verifyScrollAnchoring(page, fixture.name);
    const performance = await measureOffscreenRendering(page, context);
    const afterPerformance = await readLifecycle(stats, firstOutput);
    assertEqual(
      afterPerformance.mounts,
      initial.mounts,
      `${fixture.name} performance scroll mounts`
    );
    assertEqual(
      afterPerformance.unmounts,
      initial.unmounts,
      `${fixture.name} performance scroll unmounts`
    );
    await page.screenshot({
      path: resolve(output, `${fixture.name}.png`),
      fullPage: true
    });

    await page.getByRole('button', { name: 'exit page' }).click();
    await firstOutput.waitFor({ state: 'detached' });
    const finalUnmounts = Number(
      await stats.getAttribute('data-first-unmounts')
    );
    assertEqual(
      finalUnmounts,
      initial.unmounts + 1,
      `${fixture.name} page disposal`
    );

    if (pageErrors.length || httpErrors.length || failedRequests.length) {
      throw new Error(
        `${fixture.name} browser errors: ${JSON.stringify({ pageErrors, httpErrors, failedRequests })}`
      );
    }

    return {
      fixture: fixture.name,
      viewport: fixture.viewport,
      initial,
      transitions,
      containment,
      performance,
      anchoring,
      finalUnmounts,
      consoleErrors
    };
  } finally {
    await context.close();
  }
}

async function measureOffscreenRendering(page, context) {
  const client = await context.newCDPSession(page);
  await client.send('Performance.enable');
  try {
    const auto = await measureScrollMode(page, client, 'auto');
    const visible = await measureScrollMode(page, client, 'visible');
    return { iterations: 8, visible, auto };
  } finally {
    await client.send('Performance.disable');
    await client.detach();
  }
}

async function measureScrollMode(page, client, contentVisibility) {
  await page.evaluate((mode) => {
    for (const element of document.querySelectorAll(
      '[data-flowbase-frontstage-block-id]'
    )) {
      element.style.contentVisibility = mode;
    }
    const owner = document.querySelector(
      '[data-flowbase-frontstage-scroll-owner]'
    );
    if (owner instanceof HTMLElement) owner.scrollTop = 0;
  }, contentVisibility);
  await page.evaluate(
    () =>
      new Promise((resolvePromise) =>
        requestAnimationFrame(() => requestAnimationFrame(resolvePromise))
      )
  );
  const initiallySkippedBlockIds = await readSkippedBlockIds(page);
  await performScrollCycles(page, 1);
  const before = metricsByName(await client.send('Performance.getMetrics'));
  await performScrollCycles(page, 8);
  const after = metricsByName(await client.send('Performance.getMetrics'));
  const metrics = Object.fromEntries(
    [
      'LayoutCount',
      'LayoutDuration',
      'RecalcStyleCount',
      'RecalcStyleDuration',
      'TaskDuration'
    ].map((name) => [name, (after[name] ?? 0) - (before[name] ?? 0)])
  );
  return { ...metrics, initiallySkippedBlockIds };
}

async function readSkippedBlockIds(page) {
  return page.evaluate(() =>
    [...document.querySelectorAll('[data-flowbase-frontstage-block-id]')]
      .filter(
        (element) => !element.checkVisibility({ contentVisibilityAuto: true })
      )
      .map(
        (element) =>
          element.getAttribute('data-flowbase-frontstage-block-id') ?? ''
      )
  );
}

async function performScrollCycles(page, iterations) {
  await page.evaluate(async (count) => {
    const owner = document.querySelector(
      '[data-flowbase-frontstage-scroll-owner]'
    );
    if (!(owner instanceof HTMLElement))
      throw new Error('Scroll owner missing.');
    const frame = () => new Promise(requestAnimationFrame);
    for (let index = 0; index < count; index += 1) {
      owner.scrollTop = owner.scrollHeight;
      await frame();
      await frame();
      owner.scrollTop = 0;
      await frame();
      await frame();
    }
  }, iterations);
}

function metricsByName(result) {
  return Object.fromEntries(
    result.metrics.map(({ name, value }) => [name, value])
  );
}

async function readLifecycle(stats, firstOutput) {
  return {
    mounts: Number(await stats.getAttribute('data-first-mounts')),
    unmounts: Number(await stats.getAttribute('data-first-unmounts')),
    hookIdentity: await firstOutput.getAttribute('data-hook-identity')
  };
}

async function verifyScrollAnchoring(page, fixtureName) {
  const owner = page.locator('[data-testid=issue-1896-scroll-owner]');
  const anchor = page.locator('[data-testid=block-slot-public-a]');
  const firstSlot = page.locator('[data-testid=block-slot-first]');
  const beforeIntrinsicHeight = Number(
    await firstSlot.getAttribute('data-flowbase-frontstage-intrinsic-height')
  );

  await owner.evaluate((element) => {
    const anchorElement = element.querySelector(
      '[data-testid=block-slot-public-a]'
    );
    if (!(anchorElement instanceof HTMLElement)) {
      throw new Error('Anchor block is missing from the scroll owner.');
    }
    element.scrollTop +=
      anchorElement.getBoundingClientRect().top -
      element.getBoundingClientRect().top -
      40;
  });
  await page.evaluate(() => new Promise(requestAnimationFrame));
  const before = await readAnchorOffset(owner, anchor);

  await page
    .locator('[data-testid=native-fixture-first-output]')
    .evaluate((element) => {
      element.style.minHeight = `${Math.ceil(element.getBoundingClientRect().height) + 240}px`;
    });
  await page.waitForFunction(
    ([previousHeight]) => {
      const element = document.querySelector('[data-testid=block-slot-first]');
      return (
        Number(
          element?.getAttribute('data-flowbase-frontstage-intrinsic-height')
        ) > previousHeight
      );
    },
    [beforeIntrinsicHeight]
  );
  await page.evaluate(
    () =>
      new Promise((resolvePromise) =>
        requestAnimationFrame(() => requestAnimationFrame(resolvePromise))
      )
  );
  await page.waitForFunction(
    ([expectedOffset]) => {
      const ownerElement = document.querySelector(
        '[data-testid=issue-1896-scroll-owner]'
      );
      const anchorElement = document.querySelector(
        '[data-testid=block-slot-public-a]'
      );
      if (!(ownerElement instanceof HTMLElement) || !anchorElement)
        return false;
      return (
        Math.abs(
          anchorElement.getBoundingClientRect().top -
            ownerElement.getBoundingClientRect().top -
            expectedOffset
        ) <= 2
      );
    },
    [before.offset]
  );
  const after = await readAnchorOffset(owner, anchor);
  const drift = Math.abs(after.offset - before.offset);
  if (drift > 2) {
    throw new Error(
      `${fixtureName} scroll anchor drifted ${drift}px: ${JSON.stringify({ before, after })}`
    );
  }
  return {
    before,
    after,
    drift,
    beforeIntrinsicHeight,
    afterIntrinsicHeight: Number(
      await firstSlot.getAttribute('data-flowbase-frontstage-intrinsic-height')
    )
  };
}

async function readAnchorOffset(owner, anchor) {
  const [ownerBox, anchorBox, scrollTop] = await Promise.all([
    owner.boundingBox(),
    anchor.boundingBox(),
    owner.evaluate((element) => element.scrollTop)
  ]);
  if (!ownerBox || !anchorBox) throw new Error('Scroll anchor is not visible.');
  return { offset: anchorBox.y - ownerBox.y, scrollTop };
}

function assertEqual(actual, expected, label) {
  if (actual !== expected) {
    throw new Error(`${label}: expected ${expected}, received ${actual}`);
  }
}
