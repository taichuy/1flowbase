/* global document, Element, getComputedStyle, HTMLElement, process, requestAnimationFrame, ShadowRoot, window */
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import pageDebugCore from '../../scripts/node/page-debug/core.js';
import pageDebugAuth from '../../scripts/node/page-debug/auth.js';

const { buildChromiumLaunchOptions, loadPlaywright } = pageDebugCore;
const { loadRootCredentials, openTemporaryConsoleSession } = pageDebugAuth;
const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDirectory, '../..');
const outputDir = join(repoRoot, 'tmp/test-governance/surface-runtime-kernel');
const storageStatePath = join(outputDir, 'storage-state.json');
const pageId = '01a06a10-eebf-78c3-9cf0-4919909fda31';
const staticStylesBlockId = '01a06a11-326d-7291-afdb-aeee729183f0';
const revealBlockId = '01a06a11-33ab-7f80-b106-312b7d34f5ac';
const webBaseUrl = process.env.FLOWBASE_WEB_BASE_URL ?? 'http://127.0.0.1:3100';
const apiBaseUrl = process.env.FLOWBASE_API_BASE_URL ?? 'http://127.0.0.1:7800';
const baseUrl = `${webBaseUrl}/demo/pages/${pageId}/blocks`;

async function main() {
  mkdirSync(outputDir, { recursive: true });
  const playwright = loadPlaywright(repoRoot);
  const credentials = loadRootCredentials({ repoRoot });
  const session = await openTemporaryConsoleSession({
    playwright,
    apiBaseUrl,
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
    await context.addInitScript(() => {
      window.__surfaceScrollCalls = [];
      const nativeScrollTo = HTMLElement.prototype.scrollTo;
      HTMLElement.prototype.scrollTo = function surfaceScrollTo(...args) {
        const before = {
          left: this.scrollLeft,
          top: this.scrollTop,
          documentLeft: document.documentElement.scrollLeft,
          documentTop: document.documentElement.scrollTop
        };
        nativeScrollTo.apply(this, args);
        if (this.hasAttribute('data-flowbase-frontstage-scroll-owner')) {
          window.__surfaceScrollCalls.push({
            args,
            before,
            after: {
              left: this.scrollLeft,
              top: this.scrollTop,
              documentLeft: document.documentElement.scrollLeft,
              documentTop: document.documentElement.scrollTop
            }
          });
        }
      };
    });
    const page = await context.newPage();
    const pageErrors = [];
    const consoleErrors = [];
    page.on('pageerror', (error) => pageErrors.push(error.message));
    page.on('console', (message) => {
      if (message.type() === 'error') consoleErrors.push(message.text());
    });

    const staticStyles = await verifyStaticStylesBlock(page);
    const reveal = await verifyRevealBlock(page);
    const acceptance = {
      staticStyles,
      reveal,
      pageErrors: pageErrors.length,
      consoleErrors: consoleErrors.length
    };
    writeFileSync(
      join(outputDir, 'evidence.json'),
      `${JSON.stringify(
        {
          pageId,
          staticStylesBlockId,
          revealBlockId,
          acceptance,
          pageErrors,
          consoleErrors
        },
        null,
        2
      )}\n`
    );
    assertAcceptance(acceptance);
    process.stdout.write(`${JSON.stringify(acceptance, null, 2)}\n`);
    await context.close();
  } finally {
    await browser?.close();
    await session.dispose();
  }
}

async function verifyStaticStylesBlock(page) {
  await openReadyBlock(page, staticStylesBlockId);
  const block = page.locator(
    `[data-flowbase-frontstage-block-id="${staticStylesBlockId}"]`
  );
  const initial = await readStaticStylesState(block);
  const afterScroll = await changeOwnerScrollAndMeasure(page, block, 96);
  await page.setViewportSize({ width: 1540, height: 880 });
  await settleLayout(page);
  const afterResize = await readOverlayVectors(block);
  const scrollAlignmentError = maxVectorDelta(
    initial.overlayVectors,
    afterScroll.overlayVectors
  );
  const resizeAlignmentError = maxVectorDelta(
    afterScroll.overlayVectors,
    afterResize
  );
  await block.screenshot({
    path: join(outputDir, 'static-styles-popovers.png')
  });
  return {
    ...initial,
    ownerScrollChanged: afterScroll.afterTop !== afterScroll.beforeTop,
    documentScrollUnchanged:
      afterScroll.documentAfter.top === afterScroll.documentBefore.top &&
      afterScroll.documentAfter.left === afterScroll.documentBefore.left,
    scrollAlignmentError,
    resizeAlignmentError
  };
}

async function verifyRevealBlock(page) {
  await page.setViewportSize({ width: 1560, height: 900 });
  await openReadyBlock(page, revealBlockId);
  await settleLayout(page);
  const block = page.locator(
    `[data-flowbase-frontstage-block-id="${revealBlockId}"]`
  );
  const state = await block.evaluate((frame) => {
    const runtimeHost = frame.querySelector(
      '[data-flowbase-native-trusted-block-root]'
    );
    const root = runtimeHost?.shadowRoot;
    const button = root
      ? [...root.querySelectorAll('button')].find((candidate) =>
          candidate.textContent?.includes('Reveal In Block Surface')
        )
      : null;
    const popover = root?.querySelector('.ant-popover');
    const owner = frame.closest('[data-flowbase-frontstage-scroll-owner]');
    const visible = (element) => {
      if (!(element instanceof Element)) return false;
      const style = getComputedStyle(element);
      const rect = element.getBoundingClientRect();
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden' &&
        style.opacity !== '0'
      );
    };
    return {
      buttonVisible: visible(button),
      popoverVisible: visible(popover),
      ownerScrollTop: owner instanceof HTMLElement ? owner.scrollTop : null,
      ownerScrollLeft: owner instanceof HTMLElement ? owner.scrollLeft : null,
      documentScrollTop: document.documentElement.scrollTop,
      documentScrollLeft: document.documentElement.scrollLeft,
      surfaceScrollCalls: window.__surfaceScrollCalls ?? []
    };
  });
  const initialVectors = await readOverlayVectors(block);
  const afterScroll = await changeOwnerScrollAndMeasure(page, block, -72);
  const scrollAlignmentError = maxVectorDelta(
    initialVectors,
    afterScroll.overlayVectors
  );
  await page.setViewportSize({ width: 1540, height: 880 });
  await settleLayout(page);
  const afterResize = await readOverlayVectors(block);
  const resizeAlignmentError = maxVectorDelta(
    afterScroll.overlayVectors,
    afterResize
  );
  await block.screenshot({ path: join(outputDir, 'reveal-popover.png') });
  return {
    ...state,
    ownerScrollChanged: state.surfaceScrollCalls.some(
      (call) => call.before.top !== call.after.top
    ),
    documentScrollUnchanged:
      state.documentScrollTop === 0 &&
      state.documentScrollLeft === 0 &&
      state.surfaceScrollCalls.every(
        (call) =>
          call.before.documentTop === call.after.documentTop &&
          call.before.documentLeft === call.after.documentLeft
      ),
    scrollAlignmentError,
    resizeAlignmentError
  };
}

async function openReadyBlock(page, blockId) {
  await page.goto(`${baseUrl}/${blockId}?design=true`, {
    waitUntil: 'domcontentloaded'
  });
  const block = page.locator(
    `[data-flowbase-frontstage-block-id="${blockId}"]`
  );
  await block.waitFor({ state: 'attached', timeout: 90000 });
  if (blockId === revealBlockId) {
    await block.evaluate((frame) => {
      const owner = frame.closest('[data-flowbase-frontstage-scroll-owner]');
      if (!(owner instanceof HTMLElement)) {
        throw new Error('Frontstage scroll owner is missing.');
      }
      window.__surfaceScrollCalls = [];
      const frameRect = frame.getBoundingClientRect();
      const ownerRect = owner.getBoundingClientRect();
      owner.scrollTop = Math.max(
        0,
        owner.scrollTop + frameRect.top - ownerRect.bottom + 1
      );
    });
  } else {
    await block.scrollIntoViewIfNeeded();
  }
  await page.waitForFunction(
    (id) =>
      document
        .querySelector(`[data-flowbase-frontstage-block-id="${id}"]`)
        ?.querySelector('[data-flowbase-native-trusted-block-root]')
        ?.shadowRoot != null,
    blockId,
    { timeout: 90000 }
  );
  await page.waitForFunction(
    (id) => {
      const frame = document.querySelector(
        `[data-flowbase-frontstage-block-id="${id}"]`
      );
      return (
        frame?.getAttribute('data-flowbase-frontstage-render-status') ===
          'ready' &&
        !frame.querySelector('.frontstage-native-block-state--loading')
      );
    },
    blockId,
    { timeout: 90000 }
  );
  await settleLayout(page);
}

async function readStaticStylesState(block) {
  return block.evaluate((frame) => {
    const runtimeHost = frame.querySelector(
      '[data-flowbase-native-trusted-block-root]'
    );
    const root = runtimeHost?.shadowRoot;
    if (!root) throw new Error('Static styles Block ShadowRoot is missing.');
    const triggers = [...root.querySelectorAll('.ant-popover-open')];
    const items = triggers.map((trigger) => trigger.parentElement);
    const popovers = [...root.querySelectorAll('.ant-popover')];
    const firstItem = items[0];
    const itemClassTokens = firstItem
      ? [...firstItem.classList].filter((token) => !token.startsWith('ant-'))
      : [];
    const collectRules = (container) => {
      const styleSheets = [...container.querySelectorAll('style')]
        .map((style) => style.sheet)
        .filter(Boolean);
      if (container instanceof ShadowRoot) {
        styleSheets.push(...container.adoptedStyleSheets);
      }
      return styleSheets
        .flatMap((sheet) => {
          try {
            return [...sheet.cssRules].map((rule) => rule.cssText);
          } catch {
            return [];
          }
        })
        .join('\n');
    };
    const readCurrentVectors = () => {
      const currentTriggers = [...root.querySelectorAll('.ant-popover-open')];
      const currentPopovers = [...root.querySelectorAll('.ant-popover')];
      return currentTriggers.map((trigger, index) => {
        const triggerRect = trigger.getBoundingClientRect();
        const popupRect = currentPopovers[index]?.getBoundingClientRect();
        if (!popupRect) throw new Error(`Popover ${index} is missing.`);
        return {
          left: popupRect.left - triggerRect.left,
          top: popupRect.top - triggerRect.top
        };
      });
    };
    const shadowRules = collectRules(root);
    const headRules = collectRules(document.head);
    const itemRuleInShadow = itemClassTokens.some(
      (token) =>
        shadowRules.includes(`.${token}`) &&
        shadowRules.includes('width: 280px') &&
        shadowRules.includes('height: 280px')
    );
    const itemRuleInDocumentHead = itemClassTokens.some((token) =>
      headRules.includes(`.${token}`)
    );
    const visible = (element) => {
      if (!(element instanceof Element)) return false;
      const style = getComputedStyle(element);
      const rect = element.getBoundingClientRect();
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden' &&
        style.opacity !== '0'
      );
    };
    return {
      itemCount: items.length,
      itemSizes: items.map((item) => {
        const rect = item?.getBoundingClientRect();
        return rect ? { width: rect.width, height: rect.height } : null;
      }),
      triggerCount: triggers.length,
      visibleTriggerCount: triggers.filter(visible).length,
      popoverCount: popovers.length,
      visiblePopoverCount: popovers.filter(visible).length,
      itemRuleInShadow,
      itemRuleInDocumentHead,
      overlayVectors: readCurrentVectors()
    };
  });
}

async function changeOwnerScrollAndMeasure(page, block, delta) {
  const before = await page.evaluate((scrollDelta) => {
    const owner = document.querySelector(
      '[data-flowbase-frontstage-scroll-owner]'
    );
    if (!(owner instanceof HTMLElement)) {
      throw new Error('Frontstage scroll owner is missing.');
    }
    const result = {
      top: owner.scrollTop,
      document: {
        top: document.documentElement.scrollTop,
        left: document.documentElement.scrollLeft
      }
    };
    owner.scrollTop = Math.max(
      0,
      Math.min(
        owner.scrollHeight - owner.clientHeight,
        owner.scrollTop + scrollDelta
      )
    );
    return result;
  }, delta);
  await settleLayout(page);
  const [afterTop, documentAfter, overlayVectors] = await Promise.all([
    page
      .locator('[data-flowbase-frontstage-scroll-owner]')
      .evaluate((owner) => owner.scrollTop),
    page.evaluate(() => ({
      top: document.documentElement.scrollTop,
      left: document.documentElement.scrollLeft
    })),
    readOverlayVectors(block)
  ]);
  return {
    beforeTop: before.top,
    afterTop,
    documentBefore: before.document,
    documentAfter,
    overlayVectors
  };
}

async function readOverlayVectors(block) {
  return block.evaluate((frame) => {
    const root = frame.querySelector(
      '[data-flowbase-native-trusted-block-root]'
    )?.shadowRoot;
    if (!root) throw new Error('Native Block ShadowRoot is missing.');
    const triggers = [...root.querySelectorAll('.ant-popover-open')];
    const popovers = [...root.querySelectorAll('.ant-popover')];
    return triggers.map((trigger, index) => {
      const triggerRect = trigger.getBoundingClientRect();
      const popupRect = popovers[index]?.getBoundingClientRect();
      if (!popupRect) throw new Error(`Popover ${index} is missing.`);
      return {
        left: popupRect.left - triggerRect.left,
        top: popupRect.top - triggerRect.top
      };
    });
  });
}

async function settleLayout(page) {
  await page.evaluate(
    () =>
      new Promise((resolvePromise) =>
        requestAnimationFrame(() => requestAnimationFrame(resolvePromise))
      )
  );
}

function maxVectorDelta(before, after) {
  if (before.length === 0 || before.length !== after.length) return Infinity;
  return Math.max(
    ...before.map((vector, index) => {
      const next = after[index];
      return Math.max(
        Math.abs(vector.left - next.left),
        Math.abs(vector.top - next.top)
      );
    })
  );
}

function assertAcceptance({ staticStyles, reveal, pageErrors, consoleErrors }) {
  const allItemsAre280 = staticStyles.itemSizes.every(
    (size) => size?.width === 280 && size?.height === 280
  );
  if (
    staticStyles.itemCount !== 12 ||
    !allItemsAre280 ||
    !staticStyles.itemRuleInShadow ||
    staticStyles.itemRuleInDocumentHead ||
    staticStyles.triggerCount !== 12 ||
    staticStyles.visibleTriggerCount !== 12 ||
    staticStyles.popoverCount !== 12 ||
    staticStyles.visiblePopoverCount !== 12 ||
    !staticStyles.ownerScrollChanged ||
    !staticStyles.documentScrollUnchanged ||
    staticStyles.scrollAlignmentError > 3 ||
    staticStyles.resizeAlignmentError > 3 ||
    !reveal.buttonVisible ||
    !reveal.popoverVisible ||
    !reveal.ownerScrollChanged ||
    !reveal.documentScrollUnchanged ||
    reveal.scrollAlignmentError > 3 ||
    reveal.resizeAlignmentError > 3 ||
    pageErrors > 0 ||
    consoleErrors > 0
  ) {
    throw new Error(
      `Surface Runtime Kernel browser acceptance failed: ${JSON.stringify({
        staticStyles,
        reveal,
        pageErrors,
        consoleErrors
      })}`
    );
  }
}

main().catch((error) => {
  process.stderr.write(`${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
