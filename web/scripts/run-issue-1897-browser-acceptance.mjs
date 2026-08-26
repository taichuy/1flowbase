import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = resolve(webRoot, '..');
const output = resolve(
  repositoryRoot,
  'tmp/test-governance/issue-1897-browser'
);
const base =
  process.env.FRONTSTAGE_DRAG_FIXTURE_BASE_URL ?? 'http://127.0.0.1:4175';
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
  await page.goto(`${base}/frontstage-drag-insertion-fixture.html`, {
    waitUntil: 'networkidle'
  });
  const stats = page.locator('[data-testid=frontstage-drag-stats]');
  await stats.waitFor({ state: 'attached' });
  await assertPositions(stats, { first: 0, second: 12, saves: 0 });

  await dragBlockAcross(page, 'second', 'first', 'before');
  await assertPositions(stats, { first: 12, second: 0, saves: 1 });
  await assertDomOrder(page, ['second', 'first']);

  await page.reload({ waitUntil: 'networkidle' });
  await assertPositions(stats, { first: 12, second: 0, saves: 1 });
  await assertDomOrder(page, ['second', 'first']);

  await dragBlockAcross(page, 'second', 'first', 'after');
  await assertPositions(stats, { first: 0, second: 12, saves: 2 });
  await assertDomOrder(page, ['first', 'second']);

  await page.screenshot({
    path: resolve(output, 'desktop.png'),
    fullPage: true
  });

  if (pageErrors.length > 0) {
    throw new Error(`Page errors: ${JSON.stringify(pageErrors)}`);
  }

  const evidence = {
    ok: true,
    base,
    positions: { first: 0, second: 12 },
    saveCount: 2,
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

async function dragBlockAcross(pageInstance, activeId, targetId, side) {
  const active = gridItem(pageInstance, activeId);
  const target = gridItem(pageInstance, targetId);
  const handle = active.locator('.frontstage-block-drag-handle');
  await active.hover();
  const [handleBox, targetBox] = await Promise.all([
    handle.boundingBox(),
    target.boundingBox()
  ]);
  if (!handleBox || !targetBox) throw new Error('Drag geometry unavailable.');

  const start = {
    x: handleBox.x + handleBox.width / 2,
    y: handleBox.y + handleBox.height / 2
  };
  const end = {
    x:
      side === 'before'
        ? targetBox.x + targetBox.width * 0.25
        : targetBox.x + targetBox.width * 0.75,
    y: targetBox.y + targetBox.height / 2
  };
  await pageInstance.mouse.move(start.x, start.y);
  await pageInstance.mouse.down();
  await pageInstance.mouse.move(end.x, end.y, { steps: 12 });
  await pageInstance.mouse.up();
}

function gridItem(pageInstance, blockId) {
  return pageInstance
    .locator(`[data-testid=block-slot-${blockId}]`)
    .locator(
      'xpath=ancestor::*[contains(concat(" ", normalize-space(@class), " "), " react-grid-item ")]'
    );
}

async function assertPositions(stats, expected) {
  await stats.page().waitForFunction(({ first, second, saves }) => {
    const node = document.querySelector('[data-testid=frontstage-drag-stats]');
    return (
      node?.getAttribute('data-first-x') === String(first) &&
      node?.getAttribute('data-second-x') === String(second) &&
      node?.getAttribute('data-save-count') === String(saves)
    );
  }, expected);
}

async function assertDomOrder(pageInstance, expectedOrder) {
  await pageInstance.waitForFunction((order) => {
    const rectangles = order.map((blockId) =>
      document
        .querySelector(`[data-testid=block-slot-${blockId}]`)
        ?.closest('.react-grid-item')
        ?.getBoundingClientRect()
    );
    const [left, right] = rectangles;
    return Boolean(
      left &&
        right &&
        left.left < right.left &&
        left.right <= right.left + 1 &&
        Math.abs(left.width - right.width) <= 1
    );
  }, expectedOrder);
}
