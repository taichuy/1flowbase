/* global console, document, process */
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = resolve(webRoot, '..');
const output = resolve(
  repositoryRoot,
  'tmp/test-governance/issue-1902-browser'
);
const base =
  process.env.FRONTSTAGE_ROW_HEIGHT_FIXTURE_BASE_URL ?? 'http://127.0.0.1:4176';
const executablePath =
  process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH ?? '/usr/bin/google-chrome';

await mkdir(output, { recursive: true });
const browser = await chromium.launch({ executablePath, headless: true });
const context = await browser.newContext({
  viewport: { width: 1440, height: 900 }
});
const page = await context.newPage();
const pageErrors = [];
const consoleErrors = [];

page.on('pageerror', (error) => pageErrors.push(error.message));
page.on('console', (message) => {
  if (message.type() === 'error') consoleErrors.push(message.text());
});

try {
  await page.goto(`${base}/frontstage-equal-row-height-fixture.html`, {
    waitUntil: 'networkidle'
  });
  await page
    .locator('[data-testid=frontstage-equal-row-height-stats]')
    .waitFor({ state: 'attached' });

  await waitForEqualRow(page, 420, 200);
  const initial = await readGeometry(page);
  await page.evaluate(() => {
    document
      .querySelector('[data-testid=block-slot-short]')
      ?.setAttribute('data-stable-frame', 'short-frame');
  });

  await page.locator('[data-testid=shrink-tall-content]').click();
  await waitForEqualRow(page, 170, 200);
  const shrunken = await readGeometry(page);

  if (shrunken.rowHeight >= initial.rowHeight - 100) {
    throw new Error(
      `Row did not shrink: ${JSON.stringify({ initial, shrunken })}`
    );
  }
  if (shrunken.followingTop >= initial.followingTop - 100) {
    throw new Error(
      `Following row did not move by prefix sum: ${JSON.stringify({ initial, shrunken })}`
    );
  }
  if (!shrunken.stableFramePreserved) {
    throw new Error('First-party frame was replaced while row height changed.');
  }
  if (pageErrors.length > 0 || consoleErrors.length > 0) {
    throw new Error(
      `Browser errors: ${JSON.stringify({ pageErrors, consoleErrors })}`
    );
  }

  await page.screenshot({
    path: resolve(output, 'desktop-shrunken.png'),
    fullPage: true
  });
  await page.setViewportSize({ width: 390, height: 900 });
  const mobile = await waitForMobileStack(page);
  await page.screenshot({
    path: resolve(output, 'mobile.png'),
    fullPage: true
  });
  const evidence = {
    ok: true,
    base,
    initial,
    shrunken,
    mobile,
    pageErrors,
    consoleErrors
  };
  await writeFile(
    resolve(output, 'evidence.json'),
    `${JSON.stringify(evidence, null, 2)}\n`,
    'utf8'
  );
  console.log(JSON.stringify(evidence, null, 2));
} finally {
  await context.close();
  await browser.close();
}

async function waitForEqualRow(pageInstance, tallIntrinsic, shortIntrinsic) {
  await pageInstance.waitForFunction(
    ({ tall, short }) => {
      const shortFrame = document.querySelector(
        '[data-testid=block-slot-short]'
      );
      const tallFrame = document.querySelector('[data-testid=block-slot-tall]');
      const shortContent = document.querySelector(
        '[data-flowbase-frontstage-intrinsic-content="short"]'
      );
      const tallContent = document.querySelector(
        '[data-flowbase-frontstage-intrinsic-content="tall"]'
      );
      if (!shortFrame || !tallFrame || !shortContent || !tallContent)
        return false;
      const allocatedHeight =
        Math.ceil((Math.max(tall, short) + 10) / 3) * 3 - 10;
      return (
        Math.abs(
          shortFrame.getBoundingClientRect().height -
            tallFrame.getBoundingClientRect().height
        ) <= 1 &&
        Math.abs(shortFrame.getBoundingClientRect().height - allocatedHeight) <=
          0.5 &&
        Math.abs(shortContent.getBoundingClientRect().height - short) <= 1 &&
        Math.abs(tallContent.getBoundingClientRect().height - tall) <= 1
      );
    },
    { tall: tallIntrinsic, short: shortIntrinsic }
  );
}

async function readGeometry(pageInstance) {
  return pageInstance.evaluate(() => {
    const frame = (id) =>
      document
        .querySelector(`[data-testid=block-slot-${id}]`)
        ?.getBoundingClientRect();
    const intrinsic = (id) =>
      document
        .querySelector(`[data-flowbase-frontstage-intrinsic-content="${id}"]`)
        ?.getBoundingClientRect();
    const shortFrame = frame('short');
    const tallFrame = frame('tall');
    const followingFrame = frame('following');
    const shortContent = intrinsic('short');
    const tallContent = intrinsic('tall');
    if (
      !shortFrame ||
      !tallFrame ||
      !followingFrame ||
      !shortContent ||
      !tallContent
    ) {
      throw new Error('Equal row height geometry unavailable.');
    }
    return {
      rowHeight: shortFrame.height,
      rowHeightDifference: Math.abs(shortFrame.height - tallFrame.height),
      shortIntrinsicHeight: shortContent.height,
      tallIntrinsicHeight: tallContent.height,
      followingTop: followingFrame.top,
      stableFramePreserved:
        document
          .querySelector('[data-testid=block-slot-short]')
          ?.getAttribute('data-stable-frame') === 'short-frame'
    };
  });
}

async function waitForMobileStack(pageInstance) {
  await pageInstance.waitForFunction(() => {
    const rectangles = Array.from(document.querySelectorAll('.react-grid-item'))
      .filter((node) => !node.classList.contains('react-grid-placeholder'))
      .map((node) => node.getBoundingClientRect());
    return (
      rectangles.length === 3 &&
      rectangles.every((left, index) =>
        rectangles
          .slice(index + 1)
          .every(
            (right) =>
              left.bottom <= right.top + 1 || right.bottom <= left.top + 1
          )
      )
    );
  });
  return pageInstance.evaluate(() =>
    ['short', 'tall', 'following'].map((id) => {
      const rectangle = document
        .querySelector(`[data-testid=block-slot-${id}]`)
        ?.getBoundingClientRect();
      if (!rectangle) throw new Error(`Mobile geometry missing for ${id}.`);
      return {
        id,
        top: rectangle.top,
        width: rectangle.width,
        height: rectangle.height
      };
    })
  );
}
