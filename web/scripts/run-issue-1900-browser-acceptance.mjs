/* global console, document, localStorage, process */
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = resolve(webRoot, '..');
const output = resolve(
  repositoryRoot,
  'tmp/test-governance/issue-1900-browser'
);
const base =
  process.env.FRONTSTAGE_DRAG_FIXTURE_BASE_URL ?? 'http://127.0.0.1:4175';
const executablePath =
  process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH ?? '/usr/bin/google-chrome';
const fixtureStorageKey = 'frontstage-two-dimensional-drag-projection';

await mkdir(output, { recursive: true });
const browser = await chromium.launch({ executablePath, headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
const page = await context.newPage();
const pageErrors = [];
const consoleErrors = [];

page.on('pageerror', (error) => pageErrors.push(error.message));
page.on('console', (message) => {
  if (message.type() === 'error') consoleErrors.push(message.text());
});

try {
  await page.goto(
    `${base}/frontstage-two-dimensional-drag-projection-fixture.html`,
    { waitUntil: 'networkidle' }
  );
  await page.evaluate((key) => localStorage.removeItem(key), fixtureStorageKey);
  await page.reload({ waitUntil: 'networkidle' });
  const stats = page.locator(
    '[data-testid=frontstage-two-dimensional-drag-stats]'
  );
  await stats.waitFor({ state: 'attached' });

  const standalonePlaceholder = await dragBlockToGridPoint(
    page,
    'active',
    { column: 12, row: 64 },
    'standalone',
    false
  );
  assertPlaceholderWidth(standalonePlaceholder, 'standalone', 0.9);
  await page.mouse.up();
  await assertLayout(stats, 1, {
    first: { x: 0, y: 0, w: 12 },
    second: { x: 12, y: 0, w: 12 },
    active: { x: 0, y: 64, w: 24 },
    middle: { x: 0, y: 128, w: 24 }
  });
  await assertNoOverlap(page);

  await page.reload({ waitUntil: 'networkidle' });
  await assertLayout(stats, 1, {
    active: { x: 0, y: 64, w: 24 },
    middle: { x: 0, y: 128, w: 24 }
  });

  const joinedPlaceholder = await dragBlockToGridPoint(
    page,
    'active',
    { column: 23, row: 32 },
    'joined',
    false
  );
  assertPlaceholderWidth(joinedPlaceholder, 'joined', 0.45);
  await page.mouse.up();
  await assertLayout(stats, 2, {
    first: { x: 0, y: 0, w: 8 },
    second: { x: 8, y: 0, w: 8 },
    active: { x: 16, y: 0, w: 8 },
    middle: { x: 0, y: 64, w: 24 }
  });
  await assertNoOverlap(page);

  await page.reload({ waitUntil: 'networkidle' });
  await assertLayout(stats, 2, {
    active: { x: 16, y: 0, w: 8 },
    middle: { x: 0, y: 64, w: 24 }
  });

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
    standalonePlaceholder,
    joinedPlaceholder,
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

async function dragBlockToGridPoint(
  pageInstance,
  blockId,
  target,
  expectedKind,
  release = true
) {
  const active = gridItem(pageInstance, blockId);
  const canvas = pageInstance.locator('.frontstage-page-canvas-grid');
  await active.hover();
  const [handleBox, canvasBox] = await Promise.all([
    active.locator('.frontstage-block-drag-handle').boundingBox(),
    canvas.boundingBox()
  ]);
  if (!handleBox || !canvasBox) throw new Error('Drag geometry unavailable.');

  const start = {
    x: handleBox.x + handleBox.width / 2,
    y: handleBox.y + handleBox.height / 2
  };
  const end = {
    x: canvasBox.x + (target.column / 24) * canvasBox.width,
    y: canvasBox.y + target.row * 3
  };
  await pageInstance.mouse.move(start.x, start.y);
  await pageInstance.mouse.down();
  await pageInstance.mouse.move(end.x, end.y, { steps: 16 });
  const placeholder = pageInstance.locator('.react-grid-placeholder');
  await placeholder.waitFor({ state: 'visible' });
  await pageInstance.waitForFunction(
    ({ kind, canvasSelector }) => {
      const placeholderNode = document.querySelector('.react-grid-placeholder');
      const canvasNode = document.querySelector(canvasSelector);
      if (!placeholderNode || !canvasNode) return false;
      const ratio =
        placeholderNode.getBoundingClientRect().width /
        canvasNode.getBoundingClientRect().width;
      return kind === 'standalone' ? ratio >= 0.9 : ratio <= 0.45;
    },
    { kind: expectedKind, canvasSelector: '.frontstage-page-canvas-grid' }
  );
  const [placeholderBox, currentCanvasBox] = await Promise.all([
    placeholder.boundingBox(),
    canvas.boundingBox()
  ]);
  if (!placeholderBox || !currentCanvasBox) {
    throw new Error('Placeholder geometry unavailable.');
  }
  if (release) await pageInstance.mouse.up();
  return {
    widthRatio: placeholderBox.width / currentCanvasBox.width,
    top: placeholderBox.y - currentCanvasBox.y,
    pointerRow: (end.y - currentCanvasBox.y) / 3
  };
}

function gridItem(pageInstance, blockId) {
  return pageInstance
    .locator(`[data-testid=block-slot-${blockId}]`)
    .locator(
      'xpath=ancestor::*[contains(concat(" ", normalize-space(@class), " "), " react-grid-item ")]'
    );
}

function assertPlaceholderWidth(geometry, kind, maximumOrMinimum) {
  if (kind === 'standalone' && geometry.widthRatio < maximumOrMinimum) {
    throw new Error(`Standalone placeholder is not full width: ${JSON.stringify(geometry)}`);
  }
  if (kind === 'joined' && geometry.widthRatio > maximumOrMinimum) {
    throw new Error(`Joined placeholder is too wide: ${JSON.stringify(geometry)}`);
  }
}

async function assertLayout(stats, saveCount, expected) {
  await stats.page().waitForFunction(
    ({ saves, layouts: expectedLayouts }) => {
      const node = document.querySelector(
        '[data-testid=frontstage-two-dimensional-drag-stats]'
      );
      const layouts = JSON.parse(node?.getAttribute('data-layouts') ?? '{}');
      return (
        node?.getAttribute('data-save-count') === String(saves) &&
        Object.entries(expectedLayouts).every(([id, layout]) =>
          Object.entries(layout).every(([field, value]) =>
            layouts[id]?.[field] === value
          )
        )
      );
    },
    { saves: saveCount, layouts: expected }
  );
}

async function assertNoOverlap(pageInstance) {
  await pageInstance.waitForFunction(() => {
    const rectangles = Array.from(document.querySelectorAll('.react-grid-item'))
      .filter((node) => !node.classList.contains('react-grid-placeholder'))
      .map((node) => node.getBoundingClientRect());
    return rectangles.every((left, index) =>
      rectangles.slice(index + 1).every(
        (right) =>
          left.left >= right.right - 1 ||
          left.right <= right.left + 1 ||
          left.top >= right.bottom - 1 ||
          left.bottom <= right.top + 1
      )
    );
  });
}
