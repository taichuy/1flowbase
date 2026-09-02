/* global console, document, HTMLElement, localStorage, process, requestAnimationFrame */
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = resolve(webRoot, '..');
const output = resolve(
  repositoryRoot,
  'tmp/test-governance/issue-1899-browser'
);
const base =
  process.env.FRONTSTAGE_DRAG_FIXTURE_BASE_URL ?? 'http://127.0.0.1:4175';
const executablePath =
  process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH ?? '/usr/bin/google-chrome';
const fixtureStorageKey = 'frontstage-drag-auto-scroll-fixture';

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
  await page.goto(`${base}/frontstage-drag-auto-scroll-fixture.html`, {
    waitUntil: 'networkidle'
  });
  await page.evaluate((key) => localStorage.removeItem(key), fixtureStorageKey);
  await page.reload({ waitUntil: 'networkidle' });
  const stats = page.locator(
    '[data-testid=frontstage-drag-auto-scroll-stats]'
  );
  await stats.waitFor({ state: 'attached' });

  const upward = await dragToEdge(page, 'block-10', 'top');
  await assertSaveCount(stats, 1);
  await assertBlockAtEdge(stats, 'block-10', 'top');
  await assertNoOverlap(page);

  await page.reload({ waitUntil: 'networkidle' });
  await assertSaveCount(stats, 1);
  await assertBlockAtEdge(stats, 'block-10', 'top');

  const downward = await dragToEdge(page, 'block-10', 'bottom');
  await assertSaveCount(stats, 2);
  await assertBlockAtEdge(stats, 'block-10', 'bottom');
  await assertNoOverlap(page);

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
    upward,
    downward,
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

async function dragToEdge(pageInstance, blockId, direction) {
  const owner = pageInstance.locator('[data-testid=frontstage-scroll-owner]');
  await owner.evaluate((node, edge) => {
    node.scrollTop = edge === 'top' ? node.scrollHeight : 0;
  }, direction);
  await pageInstance.waitForTimeout(50);

  const active = gridItem(pageInstance, blockId);
  await active.hover();
  const targetScrollTop =
    direction === 'top'
      ? 0
      : await owner.evaluate(
          (node) => node.scrollHeight - node.clientHeight
        );
  const [handleBox, ownerBox] = await Promise.all([
    active.locator('.frontstage-block-drag-handle').boundingBox(),
    owner.boundingBox()
  ]);
  if (!handleBox || !ownerBox) throw new Error('Drag geometry unavailable.');

  const start = {
    x: handleBox.x + handleBox.width / 2,
    y: handleBox.y + handleBox.height / 2
  };
  const edgeY =
    direction === 'top' ? ownerBox.y + 8 : ownerBox.y + ownerBox.height - 8;
  const edgeX = ownerBox.x + 24;
  await pageInstance.mouse.move(start.x, start.y);
  await pageInstance.mouse.down();
  await pageInstance.mouse.move(edgeX, edgeY, { steps: 16 });

  const samples = await pageInstance.evaluate(
    async ({ id, pointerY, edge, scrollTarget }) => {
      const scrollOwner = document.querySelector(
        '[data-testid=frontstage-scroll-owner]'
      );
      const activeSlot = document.querySelector(`[data-testid=block-slot-${id}]`);
      const activeItem = activeSlot?.closest('.react-grid-item');
      if (!(scrollOwner instanceof HTMLElement) || !(activeItem instanceof HTMLElement)) {
        throw new Error('Drag sampling nodes unavailable.');
      }
      const values = [];
      let initialOffset = null;
      let reachedFrame = null;
      for (let frame = 0; frame < 360; frame += 1) {
        await new Promise((resolveFrame) => requestAnimationFrame(resolveFrame));
        const activeRect = activeItem.getBoundingClientRect();
        const placeholder = document.querySelector('.react-grid-placeholder');
        const placeholderRect = placeholder?.getBoundingClientRect();
        const pointerOffset = pointerY - activeRect.top;
        initialOffset ??= pointerOffset;
        values.push({
          frame,
          scrollTop: scrollOwner.scrollTop,
          pointerDrift: Math.abs(pointerOffset - initialOffset),
          placeholderTop: placeholderRect?.top ?? null
        });
        const reached =
          edge === 'top'
            ? scrollOwner.scrollTop <= 0
            : scrollOwner.scrollTop >= scrollTarget - 1;
        if (reached && reachedFrame === null) reachedFrame = frame;
        if (reachedFrame !== null && frame >= reachedFrame + 5) break;
      }
      return values;
    },
    {
      id: blockId,
      pointerY: edgeY,
      edge: direction,
      scrollTarget: targetScrollTop
    }
  );

  await pageInstance.mouse.up();
  if (samples.length === 0) throw new Error('No drag samples captured.');
  const maxPointerDrift = Math.max(...samples.map((sample) => sample.pointerDrift));
  if (maxPointerDrift > 8) {
    throw new Error(`Dragged item pointer drift exceeded 8px: ${maxPointerDrift}`);
  }
  const last = samples.at(-1);
  const scrollDirection = direction === 'top' ? -1 : 1;
  for (let index = 1; index < samples.length; index += 1) {
    const delta = samples[index].scrollTop - samples[index - 1].scrollTop;
    if (delta * scrollDirection < 0) {
      throw new Error(`Auto-scroll reversed direction at frame ${index}.`);
    }
  }
  const reachedEdge =
    direction === 'top'
      ? last.scrollTop <= 0
      : last.scrollTop >= targetScrollTop - 1;
  if (!reachedEdge) {
    const scrollMetrics = await owner.evaluate((node) => ({
      scrollTop: node.scrollTop,
      scrollHeight: node.scrollHeight,
      clientHeight: node.clientHeight
    }));
    throw new Error(
      `Auto-scroll did not reach ${direction} edge: ${JSON.stringify({ last, scrollMetrics })}`
    );
  }
  const boundarySamples = samples.slice(-5);
  if (
    boundarySamples.some(
      (sample) => Math.abs(sample.scrollTop - last.scrollTop) > 0.01
    )
  ) {
    throw new Error('Auto-scroll continued after reaching the session boundary.');
  }
  if (last.placeholderTop === null) {
    throw new Error('Insertion placeholder disappeared during edge scroll.');
  }
  return {
    direction,
    frames: samples.length,
    startScrollTop: samples[0].scrollTop,
    endScrollTop: last.scrollTop,
    targetScrollTop,
    maxPointerDrift,
    finalPlaceholderTop: last.placeholderTop
  };
}

function gridItem(pageInstance, blockId) {
  return pageInstance
    .locator(`[data-testid=block-slot-${blockId}]`)
    .locator(
      'xpath=ancestor::*[contains(concat(" ", normalize-space(@class), " "), " react-grid-item ")]'
    );
}

async function assertSaveCount(stats, expected) {
  await stats.page().waitForFunction((saveCount) => {
    const node = document.querySelector(
      '[data-testid=frontstage-drag-auto-scroll-stats]'
    );
    return node?.getAttribute('data-save-count') === String(saveCount);
  }, expected);
}

async function assertBlockAtEdge(stats, blockId, edge) {
  await stats.page().waitForFunction(
    ({ id, expectedEdge }) => {
      const node = document.querySelector(
        '[data-testid=frontstage-drag-auto-scroll-stats]'
      );
      const positions = JSON.parse(node?.getAttribute('data-positions') ?? '{}');
      const values = Object.values(positions).filter(
        (value) => typeof value === 'number'
      );
      return expectedEdge === 'top'
        ? positions[id] === Math.min(...values)
        : positions[id] === Math.max(...values);
    },
    { id: blockId, expectedEdge: edge }
  );
}

async function assertNoOverlap(pageInstance) {
  await pageInstance.waitForFunction(() => {
    const items = Array.from(document.querySelectorAll('.react-grid-item'))
      .filter((node) => !node.classList.contains('react-grid-placeholder'))
      .map((node) => node.getBoundingClientRect());
    return items.every((a, left) =>
      items.slice(left + 1).every(
        (b) =>
          a.left >= b.right - 1 ||
          a.right <= b.left + 1 ||
          a.top >= b.bottom - 1 ||
          a.bottom <= b.top + 1
      )
    );
  });
  const overlapEvidence = await pageInstance.evaluate(() => {
    const items = Array.from(document.querySelectorAll('.react-grid-item'))
      .filter((node) => !node.classList.contains('react-grid-placeholder'))
      .map((node) => ({
        id: node.querySelector('[data-testid^=block-slot-]')?.getAttribute('data-testid'),
        rect: node.getBoundingClientRect()
      }));
    const collisions = [];
    for (let left = 0; left < items.length; left += 1) {
      for (let right = left + 1; right < items.length; right += 1) {
        const a = items[left];
        const b = items[right];
        if (
          a.rect.left < b.rect.right - 1 &&
          a.rect.right > b.rect.left + 1 &&
          a.rect.top < b.rect.bottom - 1 &&
          a.rect.bottom > b.rect.top + 1
        ) {
          collisions.push([a.id, b.id]);
        }
      }
    }
    return {
      collisions,
      items: items.map(({ id, rect }) => ({
        id,
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom
      })),
      positions: JSON.parse(
        document
          .querySelector('[data-testid=frontstage-drag-auto-scroll-stats]')
          ?.getAttribute('data-positions') ?? '{}'
      )
    };
  });
  if (overlapEvidence.collisions.length > 0) {
    throw new Error(
      `Grid items overlap: ${JSON.stringify(overlapEvidence)}`
    );
  }
}
