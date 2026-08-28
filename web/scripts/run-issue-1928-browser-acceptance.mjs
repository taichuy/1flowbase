/* global document, getComputedStyle, innerHeight, innerWidth, process */
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import pageDebugCore from '../../scripts/node/page-debug/core.js';
import pageDebugAuth from '../../scripts/node/page-debug/auth.js';

const { buildChromiumLaunchOptions, loadPlaywright } = pageDebugCore;
const { loadRootCredentials, openTemporaryConsoleSession } = pageDebugAuth;
const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDirectory, '../..');
const outputDir = join(repoRoot, 'tmp/test-governance/issue-1928-browser');
const storageStatePath = join(outputDir, 'storage-state.json');
const targetUrl =
  'http://127.0.0.1:3100/demo/pages/01a047b1-4856-7c20-bac7-4dfd060b2161';
const blockId = '01a047b3-4e31-7250-93c2-c5bc73214fe8';

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
      viewport: { width: 1586, height: 1129 }
    });
    const page = await context.newPage();
    const pageErrors = [];
    page.on('pageerror', (error) => pageErrors.push(error.message));
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

    const trigger = block.getByText('Features', { exact: true });
    await trigger.waitFor({ state: 'visible', timeout: 15000 });
    await page.waitForTimeout(300);
    await trigger.hover();

    const layer = block.locator('[data-flowbase-native-overlay-layer]');
    await layer.waitFor({ state: 'attached', timeout: 15000 });
    const popup = layer.locator('[class*="menu-submenu-popup"]').first();
    await popup.waitFor({ state: 'visible', timeout: 15000 });
    await layer.getByText('Getting Started', { exact: true }).waitFor({
      state: 'visible',
      timeout: 15000
    });
    const geometry = await page.evaluate((id) => {
      const frame = document.querySelector(
        `[data-flowbase-frontstage-block-id="${id}"]`
      );
      const runtimeHost = frame?.querySelector(
        '[data-flowbase-native-trusted-block-root]'
      );
      const root = runtimeHost?.shadowRoot;
      const layerNode = root?.querySelector(
        '[data-flowbase-native-overlay-layer]'
      );
      const popupNode = layerNode?.querySelector(
        '[class*="menu-submenu-popup"]'
      );
      const triggerNode = [...(root?.querySelectorAll('*') ?? [])].find(
        (node) => node.textContent?.trim() === 'Features'
      );
      const rect = (node) => {
        const value = node?.getBoundingClientRect();
        return value
          ? {
              top: value.top,
              right: value.right,
              bottom: value.bottom,
              left: value.left,
              width: value.width,
              height: value.height
            }
          : null;
      };
      return {
        block: rect(frame),
        trigger: rect(triggerNode),
        popup: rect(popupNode),
        scrollAncestors: (() => {
          const result = [];
          let node = frame?.parentElement;
          while (node) {
            const style = getComputedStyle(node);
            if (node.scrollHeight > node.clientHeight) {
              result.push({
                tag: node.tagName,
                className: node.className,
                overflowY: style.overflowY,
                scrollTop: node.scrollTop,
                clientHeight: node.clientHeight,
                scrollHeight: node.scrollHeight
              });
            }
            node = node.parentElement;
          }
          return result;
        })(),
        layerOpen: layerNode?.matches(':popover-open') ?? false,
        layerState: layerNode?.getAttribute(
          'data-flowbase-native-overlay-state'
        ),
        popupInLayer: !!popupNode?.closest(
          '[data-flowbase-native-overlay-layer]'
        ),
        viewport: { width: innerWidth, height: innerHeight }
      };
    }, blockId);
    const acceptance = {
      pageErrors: pageErrors.length,
      layerOpen: geometry.layerOpen,
      layerState: geometry.layerState,
      popupInLayer: geometry.popupInLayer,
      popupWidth: geometry.popup?.width ?? 0,
      popupHeight: geometry.popup?.height ?? 0,
      opensBelow:
        !!geometry.popup &&
        !!geometry.trigger &&
        geometry.popup.top >= geometry.trigger.bottom - 2,
      opensAbove:
        !!geometry.popup &&
        !!geometry.trigger &&
        geometry.popup.bottom <= geometry.trigger.top + 2,
      placementFitsAvailableSpace:
        !!geometry.popup &&
        !!geometry.trigger &&
        (geometry.viewport.height - geometry.trigger.bottom >=
        geometry.popup.height
          ? geometry.popup.top >= geometry.trigger.bottom - 2
          : geometry.trigger.top >= geometry.popup.height
            ? geometry.popup.bottom <= geometry.trigger.top + 2
            : true),
      fullyInViewport:
        !!geometry.popup &&
        geometry.popup.top >= 0 &&
        geometry.popup.left >= 0 &&
        geometry.popup.right <= geometry.viewport.width &&
        geometry.popup.bottom <= geometry.viewport.height
    };
    writeFileSync(
      join(outputDir, 'evidence.json'),
      `${JSON.stringify(
        { targetUrl, blockId, geometry, acceptance, pageErrors },
        null,
        2
      )}\n`
    );
    await page.screenshot({
      path: join(outputDir, 'page.png'),
      fullPage: false
    });
    const failures = Object.entries(acceptance)
      .filter(([key, value]) =>
        key === 'pageErrors'
          ? value !== 0
          : key === 'opensBelow' || key === 'opensAbove'
            ? false
            : typeof value === 'boolean'
              ? !value
              : value <= 0
      )
      .map(([key]) => key);
    if (failures.length > 0) {
      throw new Error(
        `Issue #1928 browser acceptance failed: ${JSON.stringify({ failures, acceptance })}`
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
