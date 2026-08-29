/* global document, getComputedStyle, process */
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import pageDebugCore from '../../scripts/node/page-debug/core.js';
import pageDebugAuth from '../../scripts/node/page-debug/auth.js';

const { buildChromiumLaunchOptions, loadPlaywright } = pageDebugCore;
const { loadRootCredentials, openTemporaryConsoleSession } = pageDebugAuth;
const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDirectory, '../..');
const outputDir = join(repoRoot, 'tmp/test-governance/issue-1929-browser');
const storageStatePath = join(outputDir, 'storage-state.json');
const pageId = '01a04948-3fff-79f1-958f-bbf59c3ecfdd';
const blockId = '16cb3a93-516a-4b07-96a2-040e5df7782a';
const targetUrl = `http://127.0.0.1:3100/demo/pages/${pageId}`;

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
      viewport: { width: 1440, height: 1000 }
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
    await block.waitFor({ state: 'attached', timeout: 30000 });
    await block.scrollIntoViewIfNeeded();
    await page.waitForFunction(
      (id) =>
        document
          .querySelector(`[data-flowbase-frontstage-block-id="${id}"]`)
          ?.getAttribute('data-flowbase-frontstage-render-status') === 'ready',
      blockId,
      { timeout: 60000 }
    );

    const tabs = block.getByRole('tab');
    await tabs.first().waitFor({ state: 'visible', timeout: 15000 });
    const before = await tabs.allTextContents();
    const peerPage = await context.newPage();
    const peerPageErrors = [];
    const peerConsoleErrors = [];
    peerPage.on('pageerror', (error) => peerPageErrors.push(error.message));
    peerPage.on('console', (message) => {
      if (message.type() === 'error') peerConsoleErrors.push(message.text());
    });
    await peerPage.goto(targetUrl, { waitUntil: 'domcontentloaded' });
    const peerBlock = peerPage.locator(
      `[data-flowbase-frontstage-block-id="${blockId}"]`
    );
    await peerBlock.waitFor({ state: 'attached', timeout: 30000 });
    await peerBlock.scrollIntoViewIfNeeded();
    const peerTabs = peerBlock.getByRole('tab');
    await peerTabs.first().waitFor({ state: 'visible', timeout: 60000 });
    const peerBefore = await peerTabs.allTextContents();
    const source = block.getByRole('tab', { name: 'Tab 1', exact: true });
    const target = block.getByRole('tab', { name: 'Tab 3', exact: true });
    const sourceBox = await source.boundingBox();
    const targetBox = await target.boundingBox();
    if (!sourceBox || !targetBox) {
      throw new Error('Issue #1929 drag targets have no browser geometry.');
    }

    await page.mouse.move(
      sourceBox.x + sourceBox.width / 2,
      sourceBox.y + sourceBox.height / 2
    );
    await page.mouse.down();
    await page.mouse.move(
      sourceBox.x + sourceBox.width / 2 + 16,
      sourceBox.y + sourceBox.height / 2,
      { steps: 4 }
    );
    await page.mouse.move(
      targetBox.x + targetBox.width / 2,
      targetBox.y + targetBox.height / 2,
      { steps: 12 }
    );
    const activeTransforms = await source.evaluate((node) => {
      const transforms = [];
      let current = node;
      while (current && transforms.length < 4) {
        transforms.push({
          className: current.className,
          transform: getComputedStyle(current).transform
        });
        current = current.parentElement;
      }
      return transforms;
    });
    await page.mouse.up();
    await page.waitForFunction(
      ({ id, expected }) => {
        const frame = document.querySelector(
          `[data-flowbase-frontstage-block-id="${id}"]`
        );
        const root = frame?.querySelector(
          '[data-flowbase-native-trusted-block-root]'
        )?.shadowRoot;
        return (
          [...(root?.querySelectorAll('[role="tab"]') ?? [])]
            .map((node) => node.textContent?.trim())
            .join('|') === expected
        );
      },
      { id: blockId, expected: 'Tab 2|Tab 3|Tab 1' },
      { timeout: 15000 }
    );
    const after = await tabs.allTextContents();
    const peerAfter = await peerTabs.allTextContents();
    const geometry = {
      source: sourceBox,
      target: targetBox,
      activeTransforms
    };
    const acceptance = {
      renderReady: true,
      before,
      after,
      peerBefore,
      peerAfter,
      reordered: after.join('|') === 'Tab 2|Tab 3|Tab 1',
      peerStateIsolated:
        peerBefore.join('|') === 'Tab 1|Tab 2|Tab 3' &&
        peerAfter.join('|') === 'Tab 1|Tab 2|Tab 3',
      pointerTransformApplied: activeTransforms.some(
        ({ transform }) => transform !== 'none' && transform !== ''
      ),
      pageErrors: pageErrors.length,
      consoleErrors: consoleErrors.length,
      peerPageErrors: peerPageErrors.length,
      peerConsoleErrors: peerConsoleErrors.length
    };
    writeFileSync(
      join(outputDir, 'evidence.json'),
      `${JSON.stringify(
        {
          targetUrl,
          pageId,
          blockId,
          geometry,
          acceptance,
          pageErrors,
          consoleErrors,
          peerPageErrors,
          peerConsoleErrors
        },
        null,
        2
      )}\n`
    );
    await page.screenshot({
      path: join(outputDir, 'page.png'),
      fullPage: false
    });
    const failed =
      !acceptance.reordered ||
      !acceptance.peerStateIsolated ||
      !acceptance.pointerTransformApplied ||
      acceptance.pageErrors > 0 ||
      acceptance.consoleErrors > 0 ||
      acceptance.peerPageErrors > 0 ||
      acceptance.peerConsoleErrors > 0;
    if (failed) {
      throw new Error(
        `Issue #1929 browser acceptance failed: ${JSON.stringify(acceptance)}`
      );
    }
    process.stdout.write(
      `${JSON.stringify({ geometry, acceptance }, null, 2)}\n`
    );
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
