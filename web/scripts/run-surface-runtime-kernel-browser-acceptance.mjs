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
    geometryByRelation: {
      initial: initial.overlayVectors,
      afterScroll: afterScroll.overlayVectors,
      afterResize
    },
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
  const revealButtonName = 'Reveal In Block Surface';
  await block
    .getByRole('button', { name: revealButtonName, exact: true })
    .waitFor({ state: 'visible', timeout: 90000 });
  const relationState = await readSurfaceState(block, {
    buttonName: revealButtonName,
    includeItemStyleOwners: false
  });
  const revealRelation = relationState.relations.find(
    ({ triggerName }) => triggerName === revealButtonName
  );
  const state = await block.evaluate((frame) => {
    const owner = frame.closest('[data-flowbase-frontstage-scroll-owner]');
    return {
      ownerScrollTop: owner instanceof HTMLElement ? owner.scrollTop : null,
      ownerScrollLeft: owner instanceof HTMLElement ? owner.scrollLeft : null,
      documentScrollTop: document.documentElement.scrollTop,
      documentScrollLeft: document.documentElement.scrollLeft,
      surfaceScrollCalls: window.__surfaceScrollCalls ?? []
    };
  });
  const relationOptions = {
    buttonName: revealButtonName,
    includeItemStyleOwners: false
  };
  const initialVectors = await readOverlayVectors(block, relationOptions);
  const afterScroll = await changeOwnerScrollAndMeasure(
    page,
    block,
    -72,
    relationOptions
  );
  const scrollAlignmentError = maxVectorDelta(
    initialVectors,
    afterScroll.overlayVectors
  );
  await page.setViewportSize({ width: 1540, height: 880 });
  await settleLayout(page);
  const afterResize = await readOverlayVectors(block, relationOptions);
  const resizeAlignmentError = maxVectorDelta(
    afterScroll.overlayVectors,
    afterResize
  );
  await block.screenshot({ path: join(outputDir, 'reveal-popover.png') });
  return {
    ...state,
    surface: relationState.surface,
    relations: relationState.relations,
    geometryByRelation: {
      initial: initialVectors,
      afterScroll: afterScroll.overlayVectors,
      afterResize
    },
    buttonVisible: revealRelation?.triggerVisible ?? false,
    popoverVisible: revealRelation?.tooltipVisible ?? false,
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
        frame?.querySelector('[data-flowbase-native-trusted-block-root]')
          ?.shadowRoot != null
      );
    },
    blockId,
    { timeout: 90000 }
  );
  await settleLayout(page);
}

async function readStaticStylesState(block) {
  const state = await readSurfaceState(block, {
    includeItemStyleOwners: true
  });
  return {
    surface: state.surface,
    relations: state.relations,
    itemCount: state.items.length,
    itemSizes: state.items.map(({ geometry }) => geometry),
    triggerCount: state.relations.length,
    visibleTriggerCount: state.relations.filter(
      ({ triggerVisible }) => triggerVisible
    ).length,
    popoverCount: state.relations.length,
    visiblePopoverCount: state.relations.filter(
      ({ tooltipVisible }) => tooltipVisible
    ).length,
    itemRuleInShadow: state.items.every(
      ({ surfaceStyleOwners }) => surfaceStyleOwners.length > 0
    ),
    itemRuleInDocumentHead: state.items.some(
      ({ documentHeadStyleOwners }) => documentHeadStyleOwners.length > 0
    ),
    styleOwners: state.items.map((item) => ({
      relationIds: item.relationIds,
      surfaceStyleOwners: item.surfaceStyleOwners,
      documentHeadStyleOwners: item.documentHeadStyleOwners
    })),
    overlayVectors: state.overlayVectors
  };
}

async function changeOwnerScrollAndMeasure(
  page,
  block,
  delta,
  relationOptions = { includeItemStyleOwners: false }
) {
  const before = await block.evaluate((frame, scrollDelta) => {
    const owner = frame.closest('[data-flowbase-frontstage-scroll-owner]');
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
    block.evaluate((frame) => {
      const owner = frame.closest('[data-flowbase-frontstage-scroll-owner]');
      if (!(owner instanceof HTMLElement)) {
        throw new Error('Frontstage scroll owner is missing.');
      }
      return owner.scrollTop;
    }),
    page.evaluate(() => ({
      top: document.documentElement.scrollTop,
      left: document.documentElement.scrollLeft
    })),
    readOverlayVectors(block, relationOptions)
  ]);
  return {
    beforeTop: before.top,
    afterTop,
    documentBefore: before.document,
    documentAfter,
    overlayVectors
  };
}

async function readOverlayVectors(
  block,
  options = { includeItemStyleOwners: false }
) {
  return (await readSurfaceState(block, options)).overlayVectors;
}

async function readSurfaceState(block, options) {
  return block.evaluate((frame, readOptions) => {
    const runtimeHost = frame.querySelector(
      '[data-flowbase-native-trusted-block-root]'
    );
    const root = runtimeHost?.shadowRoot;
    if (!(root instanceof ShadowRoot)) {
      throw new Error('Native Block ShadowRoot is missing.');
    }
    const overlayHost = root.querySelector(
      '[data-flowbase-native-overlay-layer]'
    );
    if (!(overlayHost instanceof HTMLElement)) {
      throw new Error('Native Block Surface overlay host is missing.');
    }
    const frameBlockId = frame.getAttribute(
      'data-flowbase-frontstage-block-id'
    );
    const runtimeBlockId = runtimeHost.getAttribute(
      'data-flowbase-native-trusted-block-id'
    );
    const overlayBlockId = overlayHost.getAttribute(
      'data-flowbase-native-overlay-layer'
    );
    if (
      !frameBlockId ||
      runtimeBlockId !== frameBlockId ||
      overlayBlockId !== frameBlockId
    ) {
      throw new Error(
        'Native Block runtime and overlay hosts do not belong to the current Frontstage Surface.'
      );
    }

    const visible = (element) => {
      if (!(element instanceof Element)) return false;
      const style = getComputedStyle(element);
      const rect = element.getBoundingClientRect();
      return (
        rect.width > 0 &&
        rect.height > 0 &&
        style.display !== 'none' &&
        style.visibility !== 'hidden' &&
        Number.parseFloat(style.opacity || '1') > 0
      );
    };
    const accessibleName = (element) => {
      const label = element.getAttribute('aria-label');
      if (label) return label.trim();
      const labelledBy = element.getAttribute('aria-labelledby');
      if (labelledBy) {
        const labelText = labelledBy
          .split(/\s+/u)
          .map((id) => root.getElementById(id)?.textContent?.trim() ?? '')
          .filter(Boolean)
          .join(' ');
        if (labelText) return labelText;
      }
      return element.textContent?.replace(/\s+/gu, ' ').trim() ?? '';
    };
    const isButton = (element) =>
      element instanceof HTMLElement &&
      (element.localName === 'button' ||
        element.getAttribute('role') === 'button');
    const composedParent = (element) => {
      if (element.assignedSlot) return element.assignedSlot;
      if (element.parentElement) return element.parentElement;
      const ownerRoot = element.getRootNode();
      return ownerRoot instanceof ShadowRoot ? ownerRoot.host : null;
    };
    const geometry = (element) => {
      const rect = element.getBoundingClientRect();
      return {
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height
      };
    };
    const isExpectedItemGeometry = (element) => {
      const rect = element.getBoundingClientRect();
      return (
        Math.abs(rect.width - 280) <= 0.5 && Math.abs(rect.height - 280) <= 0.5
      );
    };
    const findItemOwner = (trigger) => {
      let candidate = composedParent(trigger);
      while (candidate instanceof Element) {
        if (isExpectedItemGeometry(candidate)) return candidate;
        if (candidate === runtimeHost) break;
        candidate = composedParent(candidate);
      }
      return null;
    };
    const describeSurfaceElement = (element) => ({
      tagName: element.localName,
      nativeBlockId: element.getAttribute(
        'data-flowbase-native-trusted-block-id'
      ),
      overlayLayerId: element.getAttribute('data-flowbase-native-overlay-layer')
    });

    const triggers = [...root.querySelectorAll('[aria-describedby]')].filter(
      (candidate) =>
        isButton(candidate) &&
        (!readOptions.buttonName ||
          accessibleName(candidate) === readOptions.buttonName)
    );
    const relations = triggers.flatMap((trigger) =>
      (trigger.getAttribute('aria-describedby') ?? '')
        .split(/\s+/u)
        .filter(Boolean)
        .map((relationId) => {
          const tooltip = root.getElementById(relationId);
          if (
            !(tooltip instanceof HTMLElement) ||
            tooltip.getAttribute('role') !== 'tooltip' ||
            !overlayHost.contains(tooltip)
          ) {
            throw new Error(
              `ARIA tooltip relation ${relationId} is not owned by the current Surface overlay host.`
            );
          }
          const triggerRect = trigger.getBoundingClientRect();
          const tooltipRect = tooltip.getBoundingClientRect();
          return {
            relationId,
            triggerName: accessibleName(trigger),
            triggerRole: 'button',
            triggerVisible: visible(trigger),
            tooltipRole: tooltip.getAttribute('role'),
            tooltipVisible: visible(tooltip),
            triggerGeometry: geometry(trigger),
            tooltipGeometry: geometry(tooltip),
            vector: {
              left: tooltipRect.left - triggerRect.left,
              top: tooltipRect.top - triggerRect.top
            },
            itemOwner: findItemOwner(trigger)
          };
        })
    );
    if (relations.length === 0) {
      throw new Error(
        'No stable button aria-describedby to role=tooltip relation exists in the current Surface.'
      );
    }

    const visitRules = (rules, visit) => {
      for (const rule of rules) {
        if ('selectorText' in rule && 'style' in rule) visit(rule);
        if ('cssRules' in rule) {
          try {
            visitRules(rule.cssRules, visit);
          } catch {
            // Cross-origin and disabled sheets do not expose rules.
          }
        }
      }
    };
    const matchingGeometryRules = (item, sheets) => {
      const matches = [];
      for (const { sheet, ownerType, ownerRoot } of sheets) {
        try {
          visitRules(sheet.cssRules, (rule) => {
            let selectorMatches = false;
            try {
              selectorMatches = item.matches(rule.selectorText);
            } catch {
              return;
            }
            if (
              selectorMatches &&
              Number.parseFloat(rule.style.getPropertyValue('width')) === 280 &&
              Number.parseFloat(rule.style.getPropertyValue('height')) === 280
            ) {
              matches.push({
                ownerType,
                ownerRoot,
                selector: rule.selectorText,
                width: rule.style.getPropertyValue('width'),
                height: rule.style.getPropertyValue('height')
              });
            }
          });
        } catch {
          // Cross-origin and disabled sheets do not expose rules.
        }
      }
      return matches;
    };
    const surfaceSheets = [];
    const collectShadowSheets = (shadow, path) => {
      for (const style of shadow.querySelectorAll('style')) {
        if (style.sheet) {
          surfaceSheets.push({
            sheet: style.sheet,
            ownerType: 'shadow-style-element',
            ownerRoot: path
          });
        }
      }
      for (const sheet of shadow.adoptedStyleSheets) {
        surfaceSheets.push({
          sheet,
          ownerType: 'shadow-adopted-style-sheet',
          ownerRoot: path
        });
      }
      for (const element of shadow.querySelectorAll('*')) {
        if (element.shadowRoot) {
          collectShadowSheets(
            element.shadowRoot,
            `${path}>${element.localName}::shadow-root`
          );
        }
      }
    };
    collectShadowSheets(root, 'surface::shadow-root');
    const documentHeadSheets = [
      ...document.head.querySelectorAll('style,link[rel="stylesheet"]')
    ]
      .filter((element) => element.sheet)
      .map((element) => ({
        sheet: element.sheet,
        ownerType: 'document-head-style-sheet',
        ownerRoot: 'document.head'
      }));
    const uniqueItems = new Map();
    if (readOptions.includeItemStyleOwners) {
      for (const relation of relations) {
        if (!(relation.itemOwner instanceof Element)) {
          throw new Error(
            `ARIA trigger ${relation.relationId} has no composed 280x280 item owner.`
          );
        }
        const existing = uniqueItems.get(relation.itemOwner);
        if (existing) {
          existing.relationIds.push(relation.relationId);
          continue;
        }
        uniqueItems.set(relation.itemOwner, {
          relationIds: [relation.relationId],
          geometry: geometry(relation.itemOwner),
          surfaceStyleOwners: matchingGeometryRules(
            relation.itemOwner,
            surfaceSheets
          ),
          documentHeadStyleOwners: matchingGeometryRules(
            relation.itemOwner,
            documentHeadSheets
          )
        });
      }
    }
    return {
      surface: {
        frameBlockId,
        runtimeHost: describeSurfaceElement(runtimeHost),
        overlayHost: describeSurfaceElement(overlayHost),
        relationIds: relations.map(({ relationId }) => relationId)
      },
      relations: relations.map((relation) => ({
        relationId: relation.relationId,
        triggerName: relation.triggerName,
        triggerRole: relation.triggerRole,
        triggerVisible: relation.triggerVisible,
        tooltipRole: relation.tooltipRole,
        tooltipVisible: relation.tooltipVisible,
        triggerGeometry: relation.triggerGeometry,
        tooltipGeometry: relation.tooltipGeometry,
        vector: relation.vector
      })),
      items: [...uniqueItems.values()],
      overlayVectors: relations.map(({ relationId, vector }) => ({
        relationId,
        ...vector
      }))
    };
  }, options);
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
  const afterByRelationId = new Map(
    after.map((vector) => [vector.relationId, vector])
  );
  return Math.max(
    ...before.map((vector) => {
      const next = afterByRelationId.get(vector.relationId);
      if (!next) return Infinity;
      return Math.max(
        Math.abs(vector.left - next.left),
        Math.abs(vector.top - next.top)
      );
    })
  );
}

function assertAcceptance({ staticStyles, reveal, pageErrors, consoleErrors }) {
  const allItemsAre280 = staticStyles.itemSizes.every(
    (size) =>
      Math.abs((size?.width ?? 0) - 280) <= 0.5 &&
      Math.abs((size?.height ?? 0) - 280) <= 0.5
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
    reveal.relations.length !== 1 ||
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
