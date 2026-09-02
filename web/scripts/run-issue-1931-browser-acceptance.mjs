/* global document, process */
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import pageDebugCore from '../../scripts/node/page-debug/core.js';
import pageDebugAuth from '../../scripts/node/page-debug/auth.js';

const { buildChromiumLaunchOptions, loadPlaywright } = pageDebugCore;
const { loadRootCredentials, openTemporaryConsoleSession } = pageDebugAuth;
const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDirectory, '../..');
const outputDir = join(repoRoot, 'tmp/test-governance/issue-1931-browser');
const storageStatePath = join(outputDir, 'storage-state.json');
const pageId = '01a04b56-e1b0-70f1-864b-6984b46022c2';
const blockId = '2b6e34f3-2d9c-4504-a625-32305f312f0a';
const targetUrl = `http://127.0.0.1:3100/demo/pages/${pageId}/blocks/${blockId}?design=true`;

async function main() {
  mkdirSync(outputDir, { recursive: true });
  const playwright = loadPlaywright(repoRoot);
  const credentials = loadRootCredentials({ repoRoot });
  const session = await openTemporaryConsoleSession({
    playwright,
    apiBaseUrl: 'http://127.0.0.1:7800',
    account: credentials.account,
    password: credentials.password,
    storageStatePath
  });
  let browser;
  try {
    browser = await playwright.chromium.launch(
      buildChromiumLaunchOptions({ headless: true })
    );
    const context = await browser.newContext({
      storageState: storageStatePath,
      viewport: { width: 1560, height: 900 }
    });
    const page = await context.newPage();
    const pageErrors = [];
    const consoleErrors = [];
    page.on('pageerror', (error) => pageErrors.push(error.message));
    page.on('console', (message) => {
      if (message.type() === 'error') consoleErrors.push(message.text());
    });
    await page.goto(targetUrl, { waitUntil: 'domcontentloaded' });

    const block = page.locator(
      `[data-flowbase-frontstage-block-id="${blockId}"]`
    );
    await block.waitFor({ state: 'attached', timeout: 90000 });
    await block.scrollIntoViewIfNeeded();
    await page.waitForFunction(
      (id) =>
        document
          .querySelector(`[data-flowbase-frontstage-block-id="${id}"]`)
          ?.getAttribute('data-flowbase-frontstage-render-status') === 'ready',
      blockId,
      { timeout: 90000 }
    );

    const openCascader = async () => {
      await block.locator('input').first().click();
      const layer = block.locator(
        `[data-flowbase-native-overlay-layer="${blockId}"]`
      );
      await layer.waitFor({ state: 'attached', timeout: 15000 });
      await page.waitForFunction(
        (id) => {
          const frame = document.querySelector(
            `[data-flowbase-frontstage-block-id="${id}"]`
          );
          const root = frame?.querySelector(
            '[data-flowbase-native-trusted-block-root]'
          )?.shadowRoot;
          return (
            root
              ?.querySelector(`[data-flowbase-native-overlay-layer="${id}"]`)
              ?.getAttribute('data-flowbase-native-overlay-state') === 'open'
          );
        },
        blockId,
        { timeout: 15000 }
      );
      await layer.getByText('Zhejiang (zhejiang)', { exact: true }).waitFor({
        state: 'visible',
        timeout: 15000
      });
      return layer;
    };

    const firstLayer = await openCascader();
    const popup = firstLayer.locator('[class*="-cascader-menus"]').first();
    const [blockBox, popupBox, topLayerOpen] = await Promise.all([
      block.boundingBox(),
      popup.boundingBox(),
      firstLayer.evaluate((element) => element.matches(':popover-open'))
    ]);
    if (!blockBox || !popupBox) {
      throw new Error('Issue #1931 Block or Cascader popup has no geometry.');
    }
    const crossesBlockBoundary =
      popupBox.x < blockBox.x ||
      popupBox.y < blockBox.y ||
      popupBox.x + popupBox.width > blockBox.x + blockBox.width ||
      popupBox.y + popupBox.height > blockBox.y + blockBox.height;

    await page.screenshot({
      path: join(outputDir, 'cascader-top-layer.png'),
      fullPage: false
    });

    const designToggle = page.getByRole('button', {
      name: /进入设计模式|退出设计模式/u
    });
    await designToggle.click();
    await page.waitForFunction(
      (id) => {
        const frame = document.querySelector(
          `[data-flowbase-frontstage-block-id="${id}"]`
        );
        const root = frame?.querySelector(
          '[data-flowbase-native-trusted-block-root]'
        )?.shadowRoot;
        return (
          root
            ?.querySelector(`[data-flowbase-native-overlay-layer="${id}"]`)
            ?.getAttribute('data-flowbase-native-overlay-state') === 'closed'
        );
      },
      blockId,
      { timeout: 15000 }
    );
    await designToggle.click();
    const secondLayer = await openCascader();
    const reopenedAfterModeRoundTrip =
      (await secondLayer.getAttribute('data-flowbase-native-overlay-state')) ===
      'open';

    const acceptance = {
      renderReady: true,
      crossesBlockBoundary,
      topLayerOpen,
      reopenedAfterModeRoundTrip,
      blockBox,
      popupBox,
      pageErrors: pageErrors.length,
      consoleErrors: consoleErrors.length
    };
    writeFileSync(
      join(outputDir, 'evidence.json'),
      `${JSON.stringify(
        { targetUrl, pageId, blockId, acceptance, pageErrors, consoleErrors },
        null,
        2
      )}\n`
    );
    if (
      !crossesBlockBoundary ||
      !topLayerOpen ||
      !reopenedAfterModeRoundTrip ||
      pageErrors.length > 0 ||
      consoleErrors.length > 0
    ) {
      throw new Error(
        `Issue #1931 browser acceptance failed: ${JSON.stringify(acceptance)}`
      );
    }
    process.stdout.write(`${JSON.stringify(acceptance, null, 2)}\n`);
    await context.close();
  } finally {
    await browser?.close();
    await session.dispose();
  }
}

main().catch((error) => {
  process.stderr.write(`${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
