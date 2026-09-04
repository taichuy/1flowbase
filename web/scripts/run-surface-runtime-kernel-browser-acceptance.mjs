/* global document, Element, getComputedStyle, HTMLElement, process, requestAnimationFrame, ShadowRoot, window */
import {
  existsSync,
  mkdirSync,
  readdirSync,
  renameSync,
  unlinkSync,
  writeFileSync
} from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import pageDebugCore from '../../scripts/node/page-debug/core.js';
import pageDebugAuth from '../../scripts/node/page-debug/auth.js';

const { buildChromiumLaunchOptions, loadPlaywright } = pageDebugCore;
const { loadRootCredentials, openTemporaryConsoleSession } = pageDebugAuth;
const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDirectory, '../..');
const runId = `${new Date().toISOString().replace(/[:.]/gu, '-')}-${process.pid}`;
const outputDir = process.env.SURFACE_RUNTIME_KERNEL_OUTPUT_DIR
  ? resolve(repoRoot, process.env.SURFACE_RUNTIME_KERNEL_OUTPUT_DIR)
  : join(
      repoRoot,
      'tmp/test-governance/surface-runtime-kernel',
      `run-${runId}`
    );
const storageStatePath = join(outputDir, 'storage-state.json');
const storageStateTemporaryPath = join(
  outputDir,
  `.storage-state.temporary-${process.pid}.json`
);
const evidencePath = join(outputDir, 'evidence.json');
const staticScreenshotPath = join(outputDir, 'static-styles-popovers.png');
const revealScreenshotPath = join(outputDir, 'reveal-popover.png');
const pageId = '01a06a10-eebf-78c3-9cf0-4919909fda31';
const staticStylesBlockId = '01a06a11-326d-7291-afdb-aeee729183f0';
const revealBlockId = '01a06a11-33ab-7f80-b106-312b7d34f5ac';
const expectedStaticPairCount = 12;
const minimumAssignmentMarginSquaredPixels = 9;
const webBaseUrl = process.env.FLOWBASE_WEB_BASE_URL ?? 'http://127.0.0.1:3100';
const apiBaseUrl = process.env.FLOWBASE_API_BASE_URL ?? 'http://127.0.0.1:7800';
const baseUrl = `${webBaseUrl}/demo/pages/${pageId}/blocks`;

async function main() {
  if (existsSync(outputDir) && readdirSync(outputDir).length > 0) {
    throw new Error(
      `Surface Runtime Kernel output directory must be empty: ${outputDir}`
    );
  }
  mkdirSync(outputDir, { recursive: true });
  const evidence = {
    schemaVersion: 1,
    run: {
      id: runId,
      startedAt: new Date().toISOString(),
      completedAt: null,
      outputDir
    },
    pageId,
    staticStylesBlockId,
    revealBlockId,
    phases: {
      Static: pendingPhaseEvidence({ screenshot: staticScreenshotPath }),
      Reveal: pendingPhaseEvidence({ screenshot: revealScreenshotPath })
    }
  };
  atomicWriteJson(evidencePath, evidence);
  const playwright = loadPlaywright(repoRoot);
  const credentials = loadRootCredentials({ repoRoot });
  const session = await openTemporaryConsoleSession({
    playwright,
    apiBaseUrl,
    account: credentials.account,
    password: credentials.password,
    storageStatePath: storageStateTemporaryPath
  });
  let browser;
  try {
    renameSync(storageStateTemporaryPath, storageStatePath);
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
    page.on('pageerror', (error) =>
      pageErrors.push({
        timestamp: new Date().toISOString(),
        message: error.message
      })
    );
    page.on('console', (message) => {
      if (message.type() === 'error') {
        consoleErrors.push({
          timestamp: new Date().toISOString(),
          message: message.text()
        });
      }
    });

    const staticStyles = await runEvidencePhase({
      evidence,
      phase: 'Static',
      pageErrors,
      consoleErrors,
      run: () => verifyStaticStylesBlock(page),
      assertResult: assertStaticAcceptance
    });
    const reveal = await runEvidencePhase({
      evidence,
      phase: 'Reveal',
      pageErrors,
      consoleErrors,
      run: () => verifyRevealBlock(page),
      assertResult: assertRevealAcceptance,
      collectFailureEvidence: () => readRevealScrollChannels(page)
    });
    const acceptance = { staticStyles, reveal };
    evidence.run.completedAt = new Date().toISOString();
    atomicWriteJson(evidencePath, evidence);
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
  const relationOptions = {
    mode: 'tracked-static',
    pairs: initial.pairs.map(({ itemProbeId, tooltipContent }) => ({
      itemProbeId,
      tooltipContent
    }))
  };
  const afterScroll = await changeOwnerScrollAndMeasure(
    page,
    block,
    96,
    relationOptions
  );
  await page.setViewportSize({ width: 1540, height: 880 });
  await settleLayout(page);
  const afterResize = await readOverlayVectors(block, relationOptions);
  const scrollAlignmentError = maxVectorDelta(
    initial.overlayVectors,
    afterScroll.overlayVectors
  );
  const resizeAlignmentError = maxVectorDelta(
    afterScroll.overlayVectors,
    afterResize
  );
  await atomicScreenshot(block, staticScreenshotPath);
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
    resizeAlignmentError,
    artifacts: { screenshot: staticScreenshotPath }
  };
}

async function verifyRevealBlock(page) {
  await page.setViewportSize({ width: 1560, height: 900 });
  const { fixtureMountOperations } = await openReadyBlock(page, revealBlockId);
  await settleLayout(page);
  const block = page.locator(
    `[data-flowbase-frontstage-block-id="${revealBlockId}"]`
  );
  const revealButtonName = 'Reveal In Block Surface';
  await block
    .getByRole('button', { name: revealButtonName, exact: true })
    .waitFor({ state: 'visible', timeout: 90000 });
  const relationOptions = { mode: 'reveal', buttonName: revealButtonName };
  const relationState = await readSurfaceState(block, relationOptions);
  const revealRelation = relationState.relations[0];
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
  const initialVectors = relationState.overlayVectors;
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
  await atomicScreenshot(block, revealScreenshotPath);
  return {
    ...state,
    fixtureMountOperations,
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
    resizeAlignmentError,
    artifacts: { screenshot: revealScreenshotPath }
  };
}

async function readRevealScrollChannels(page) {
  return page.evaluate(() => ({
    fixtureMountOperations: window.__fixtureMountOperations ?? [],
    surfaceScrollCalls: window.__surfaceScrollCalls ?? []
  }));
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
    await block.evaluate(() => {
      window.__surfaceScrollCalls = [];
      window.__fixtureMountOperations = [];
    });
    await page.waitForFunction(
      (id) => {
        const frame = document.querySelector(
          `[data-flowbase-frontstage-block-id="${id}"]`
        );
        if (!(frame instanceof HTMLElement)) return false;
        const runtimeHost = frame.querySelector(
          '[data-flowbase-native-trusted-block-root]'
        );
        const materialized =
          frame.getAttribute('data-flowbase-frontstage-render-status') ===
            'ready' && runtimeHost?.shadowRoot != null;
        if (materialized) return true;

        const owner = frame.closest('[data-flowbase-frontstage-scroll-owner]');
        if (!(owner instanceof HTMLElement)) {
          throw new Error('Frontstage scroll owner is missing.');
        }
        const frameRect = frame.getBoundingClientRect();
        const ownerRect = owner.getBoundingClientRect();
        const edgeVisibility = 16;
        if (frameRect.height <= edgeVisibility) {
          throw new Error(
            'Reveal Block is too short for a partial edge-pin mount fixture.'
          );
        }
        const desiredTop = ownerRect.bottom - edgeVisibility;
        const maximumScrollTop = Math.max(
          0,
          owner.scrollHeight - owner.clientHeight
        );
        const nextScrollTop = Math.max(
          0,
          Math.min(
            maximumScrollTop,
            owner.scrollTop + frameRect.top - desiredTop
          )
        );
        if (Math.abs(nextScrollTop - owner.scrollTop) <= 0.5) {
          const visibleHeight = Math.max(
            0,
            Math.min(frameRect.bottom, ownerRect.bottom) -
              Math.max(frameRect.top, ownerRect.top)
          );
          if (visibleHeight >= frameRect.height - 0.5) {
            throw new Error(
              'Reveal mount fixture found the target fully visible before materialization.'
            );
          }
          return false;
        }

        const beforeTop = owner.scrollTop;
        const documentBefore = {
          left: document.documentElement.scrollLeft,
          top: document.documentElement.scrollTop
        };
        owner.scrollTop = nextScrollTop;
        const pinnedRect = frame.getBoundingClientRect();
        const visibleHeight = Math.max(
          0,
          Math.min(pinnedRect.bottom, ownerRect.bottom) -
            Math.max(pinnedRect.top, ownerRect.top)
        );
        window.__fixtureMountOperations.push({
          timestamp: new Date().toISOString(),
          beforeTop,
          requestedTop: nextScrollTop,
          afterTop: owner.scrollTop,
          targetTop: pinnedRect.top,
          ownerTop: ownerRect.top,
          ownerBottom: ownerRect.bottom,
          targetHeight: pinnedRect.height,
          visibleHeight,
          documentBefore,
          documentAfter: {
            left: document.documentElement.scrollLeft,
            top: document.documentElement.scrollTop
          }
        });
        if (visibleHeight >= pinnedRect.height - 0.5) {
          throw new Error(
            'Reveal mount fixture fully scrolled the target into the owner viewport.'
          );
        }
        return false;
      },
      blockId,
      { polling: 'raf', timeout: 90000 }
    );
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
  return blockId === revealBlockId
    ? block.evaluate(() => ({
        fixtureMountOperations: window.__fixtureMountOperations ?? []
      }))
    : { fixtureMountOperations: [] };
}

async function readStaticStylesState(block) {
  const state = await readSurfaceState(block, { mode: 'discover-static' });
  if (
    state.items.length !== expectedStaticPairCount ||
    state.tooltips.length !== expectedStaticPairCount
  ) {
    throw new Error(
      `Static Surface requires a ${expectedStaticPairCount}x${expectedStaticPairCount} geometry matrix; found ${state.items.length} items and ${state.tooltips.length} tooltips.`
    );
  }
  const tooltipContents = state.tooltips.map(({ content }) => content);
  if (
    tooltipContents.some((content) => content.length === 0) ||
    new Set(tooltipContents).size !== tooltipContents.length
  ) {
    throw new Error(
      'Static Surface role=tooltip content must be non-empty and globally unique.'
    );
  }
  const itemProbeIds = state.items.map(({ probeId }) => probeId);
  if (new Set(itemProbeIds).size !== itemProbeIds.length) {
    throw new Error('Static Surface item DOM identities are not unique.');
  }
  const costMatrix = state.items.map((item) =>
    state.tooltips.map((tooltip) => centerDistanceSquared(item, tooltip))
  );
  const optimal = solveAssignment(costMatrix);
  if (!Number.isFinite(optimal.cost)) {
    throw new Error(
      'Static Surface optimal geometry assignment is not finite.'
    );
  }
  let secondBestCost = Infinity;
  for (const [itemIndex, tooltipIndex] of optimal.assignment.entries()) {
    const withoutMatchedEdge = costMatrix.map((row) => [...row]);
    withoutMatchedEdge[itemIndex][tooltipIndex] = Infinity;
    secondBestCost = Math.min(
      secondBestCost,
      solveAssignment(withoutMatchedEdge).cost
    );
  }
  const ambiguityMargin = secondBestCost - optimal.cost;
  if (
    !Number.isFinite(secondBestCost) ||
    ambiguityMargin < minimumAssignmentMarginSquaredPixels
  ) {
    throw new Error(
      `Static Surface geometry assignment is ambiguous: best=${optimal.cost}, secondBest=${secondBestCost}, margin=${ambiguityMargin}, requiredMargin=${minimumAssignmentMarginSquaredPixels}.`
    );
  }
  const pairs = optimal.assignment.map((tooltipIndex, itemIndex) => {
    const item = state.items[itemIndex];
    const tooltip = state.tooltips[tooltipIndex];
    return {
      relationKey: `${item.probeId}:${tooltip.content}`,
      itemProbeId: item.probeId,
      tooltipContent: tooltip.content,
      tooltipRole: tooltip.role,
      tooltipId: tooltip.id,
      costSquaredPixels: costMatrix[itemIndex][tooltipIndex],
      itemGeometry: item.geometry,
      tooltipGeometry: tooltip.geometry,
      vector: centerVector(item, tooltip)
    };
  });
  return {
    surface: state.surface,
    matching: {
      algorithm: 'hungarian-exact-minimum-cost-assignment',
      matrixShape: [state.items.length, state.tooltips.length],
      optimalCostSquaredPixels: optimal.cost,
      secondBestCostSquaredPixels: secondBestCost,
      ambiguityMarginSquaredPixels: ambiguityMargin,
      requiredMarginSquaredPixels: minimumAssignmentMarginSquaredPixels
    },
    pairs,
    itemCount: state.items.length,
    itemContentBoxes: state.items.map(
      ({ computedContentBox }) => computedContentBox
    ),
    itemBorderBoxes: state.items.map(({ borderBox }) => borderBox),
    popoverCount: state.tooltips.length,
    visiblePopoverCount: state.tooltips.filter(({ visible }) => visible).length,
    itemRuleInShadow: state.items.every(
      ({ surfaceStyleOwners }) => surfaceStyleOwners.length > 0
    ),
    itemRuleInDocumentHead: state.items.some(
      ({ documentHeadStyleOwners }) => documentHeadStyleOwners.length > 0
    ),
    styleOwners: state.items.map((item) => ({
      itemProbeId: item.probeId,
      authoredRuleSize: item.authoredRuleSize,
      computedContentBox: item.computedContentBox,
      borderBox: item.borderBox,
      boxSizing: item.boxSizing,
      border: item.border,
      padding: item.padding,
      surfaceStyleOwners: item.surfaceStyleOwners,
      documentHeadStyleOwners: item.documentHeadStyleOwners
    })),
    overlayVectors: pairs.map(
      ({ relationKey, itemGeometry, tooltipGeometry, vector }) => ({
        relationKey,
        itemGeometry,
        tooltipGeometry,
        ...vector
      })
    )
  };
}

async function changeOwnerScrollAndMeasure(
  page,
  block,
  delta,
  relationOptions
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

async function readOverlayVectors(block, options) {
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
    const normalizeContent = (value) =>
      value?.replace(/\s+/gu, ' ').trim() ?? '';
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
      return normalizeContent(element.textContent);
    };
    const isButton = (element) =>
      element instanceof HTMLElement &&
      (element.localName === 'button' ||
        element.getAttribute('role') === 'button');
    const geometry = (element) => {
      const rect = element.getBoundingClientRect();
      return {
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height
      };
    };
    const parsePixelValue = (value) => {
      const match = value.trim().match(/^(-?(?:\d+\.?\d*|\.\d+))px$/u);
      return match ? Number.parseFloat(match[1]) : Number.NaN;
    };
    const boxModel = (element) => {
      const style = getComputedStyle(element);
      const padding = {
        top: parsePixelValue(style.paddingTop),
        right: parsePixelValue(style.paddingRight),
        bottom: parsePixelValue(style.paddingBottom),
        left: parsePixelValue(style.paddingLeft)
      };
      const border = {
        top: parsePixelValue(style.borderTopWidth),
        right: parsePixelValue(style.borderRightWidth),
        bottom: parsePixelValue(style.borderBottomWidth),
        left: parsePixelValue(style.borderLeftWidth)
      };
      const computedWidth = parsePixelValue(style.width);
      const computedHeight = parsePixelValue(style.height);
      const horizontalInsets =
        padding.left + padding.right + border.left + border.right;
      const verticalInsets =
        padding.top + padding.bottom + border.top + border.bottom;
      const computedContentBox = {
        width:
          style.boxSizing === 'border-box'
            ? computedWidth - horizontalInsets
            : computedWidth,
        height:
          style.boxSizing === 'border-box'
            ? computedHeight - verticalInsets
            : computedHeight
      };
      return {
        computedContentBox,
        borderBox: geometry(element),
        boxSizing: style.boxSizing,
        border,
        padding
      };
    };
    const centerVector = (source, target) => {
      const sourceRect = source.getBoundingClientRect();
      const targetRect = target.getBoundingClientRect();
      return {
        left:
          targetRect.left +
          targetRect.width / 2 -
          (sourceRect.left + sourceRect.width / 2),
        top:
          targetRect.top +
          targetRect.height / 2 -
          (sourceRect.top + sourceRect.height / 2)
      };
    };
    const describeSurfaceElement = (element) => ({
      tagName: element.localName,
      nativeBlockId: element.getAttribute(
        'data-flowbase-native-trusted-block-id'
      ),
      overlayLayerId: element.getAttribute('data-flowbase-native-overlay-layer')
    });

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
            const width = rule.style.getPropertyValue('width');
            const height = rule.style.getPropertyValue('height');
            if (
              selectorMatches &&
              parsePixelValue(width) === 280 &&
              parsePixelValue(height) === 280
            ) {
              matches.push({
                ownerType,
                ownerRoot,
                width,
                height
              });
            }
          });
        } catch {
          // Cross-origin and disabled sheets do not expose rules.
        }
      }
      return matches;
    };
    const shadowScopes = [];
    const collectShadowSheets = (shadow, path) => {
      const sheets = [];
      for (const style of shadow.querySelectorAll('style')) {
        if (style.getRootNode() === shadow && style.sheet) {
          sheets.push({
            sheet: style.sheet,
            ownerType: 'shadow-style-element',
            ownerRoot: path
          });
        }
      }
      for (const sheet of shadow.adoptedStyleSheets) {
        sheets.push({
          sheet,
          ownerType: 'shadow-adopted-style-sheet',
          ownerRoot: path
        });
      }
      shadowScopes.push({ shadow, sheets });
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
    const surface = {
      frameBlockId,
      runtimeHost: describeSurfaceElement(runtimeHost),
      overlayHost: describeSurfaceElement(overlayHost)
    };
    const visibleTooltips = [
      ...overlayHost.querySelectorAll('[role="tooltip"]')
    ]
      .filter((element) => visible(element))
      .map((element) => ({
        element,
        content: normalizeContent(element.innerText || element.textContent),
        id: element.id || null,
        role: element.getAttribute('role'),
        geometry: geometry(element),
        visible: true
      }));

    if (readOptions.mode === 'reveal') {
      const buttons = [
        ...root.querySelectorAll('button,[role="button"]')
      ].filter(
        (element) =>
          isButton(element) &&
          visible(element) &&
          accessibleName(element) === readOptions.buttonName
      );
      if (buttons.length !== 1) {
        throw new Error(
          `Reveal Surface requires one visible role=button named ${readOptions.buttonName}; found ${buttons.length}.`
        );
      }
      if (
        visibleTooltips.length !== 1 ||
        visibleTooltips[0].content.length === 0
      ) {
        throw new Error(
          `Reveal Surface requires one visible role=tooltip with unique non-empty content; found ${visibleTooltips.length}.`
        );
      }
      const button = buttons[0];
      const tooltip = visibleTooltips[0];
      const relationKey = `reveal:${tooltip.content}`;
      const vector = centerVector(button, tooltip.element);
      return {
        surface,
        relations: [
          {
            relationKey,
            triggerName: accessibleName(button),
            triggerRole: 'button',
            triggerVisible: true,
            tooltipContent: tooltip.content,
            tooltipId: tooltip.id,
            tooltipRole: tooltip.role,
            tooltipVisible: true,
            triggerGeometry: geometry(button),
            tooltipGeometry: tooltip.geometry,
            vector
          }
        ],
        overlayVectors: [
          {
            relationKey,
            itemGeometry: geometry(button),
            tooltipGeometry: tooltip.geometry,
            ...vector
          }
        ]
      };
    }

    window.__surfaceGeometryProbeState ??= {
      itemIds: new WeakMap(),
      itemsById: new Map(),
      nextItemId: 1
    };
    const probeState = window.__surfaceGeometryProbeState;
    if (readOptions.mode === 'discover-static') {
      const items = [];
      for (const { shadow, sheets } of shadowScopes) {
        for (const element of shadow.querySelectorAll('*')) {
          if (overlayHost.contains(element)) continue;
          if (!visible(element)) continue;
          const surfaceStyleOwners = matchingGeometryRules(element, sheets);
          if (surfaceStyleOwners.length === 0) continue;
          const metrics = boxModel(element);
          if (
            !Number.isFinite(metrics.computedContentBox.width) ||
            !Number.isFinite(metrics.computedContentBox.height) ||
            Math.abs(metrics.computedContentBox.width - 280) > 0.5 ||
            Math.abs(metrics.computedContentBox.height - 280) > 0.5
          ) {
            continue;
          }
          let probeId = probeState.itemIds.get(element);
          if (!probeId) {
            probeId = `surface-item-${probeState.nextItemId}`;
            probeState.nextItemId += 1;
            probeState.itemIds.set(element, probeId);
            probeState.itemsById.set(probeId, element);
          }
          items.push({
            probeId,
            geometry: metrics.borderBox,
            authoredRuleSize: surfaceStyleOwners.map(({ width, height }) => ({
              width,
              height
            })),
            computedContentBox: metrics.computedContentBox,
            borderBox: metrics.borderBox,
            boxSizing: metrics.boxSizing,
            border: metrics.border,
            padding: metrics.padding,
            surfaceStyleOwners,
            documentHeadStyleOwners: matchingGeometryRules(
              element,
              documentHeadSheets
            )
          });
        }
      }
      return {
        surface,
        items,
        tooltips: visibleTooltips.map(
          ({
            content,
            id,
            role,
            geometry: tooltipGeometry,
            visible: isVisible
          }) => ({
            content,
            id,
            role,
            geometry: tooltipGeometry,
            visible: isVisible
          })
        )
      };
    }

    if (readOptions.mode !== 'tracked-static') {
      throw new Error(
        `Unknown Surface geometry read mode: ${readOptions.mode}`
      );
    }
    const tooltipByContent = new Map();
    for (const tooltip of visibleTooltips) {
      if (!tooltip.content || tooltipByContent.has(tooltip.content)) {
        throw new Error(
          'Tracked Static Surface tooltip content is empty or no longer unique.'
        );
      }
      tooltipByContent.set(tooltip.content, tooltip);
    }
    return {
      surface,
      overlayVectors: readOptions.pairs.map(
        ({ itemProbeId, tooltipContent }) => {
          const item = probeState.itemsById.get(itemProbeId);
          const tooltip = tooltipByContent.get(tooltipContent);
          if (
            !(item instanceof Element) ||
            probeState.itemIds.get(item) !== itemProbeId ||
            !item.isConnected
          ) {
            throw new Error(
              `Tracked Static Surface item ${itemProbeId} is not the original DOM element.`
            );
          }
          if (!tooltip) {
            throw new Error(
              `Tracked Static Surface tooltip content disappeared: ${tooltipContent}`
            );
          }
          const vector = centerVector(item, tooltip.element);
          return {
            relationKey: `${itemProbeId}:${tooltipContent}`,
            itemGeometry: geometry(item),
            tooltipGeometry: tooltip.geometry,
            ...vector
          };
        }
      )
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

function centerVector(source, target) {
  return {
    left:
      target.geometry.left +
      target.geometry.width / 2 -
      (source.geometry.left + source.geometry.width / 2),
    top:
      target.geometry.top +
      target.geometry.height / 2 -
      (source.geometry.top + source.geometry.height / 2)
  };
}

function centerDistanceSquared(source, target) {
  const vector = centerVector(source, target);
  return vector.left * vector.left + vector.top * vector.top;
}

function solveAssignment(costMatrix) {
  const size = costMatrix.length;
  if (size === 0 || costMatrix.some((row) => row.length !== size)) {
    return { assignment: [], cost: Infinity };
  }
  const rowPotential = Array(size + 1).fill(0);
  const columnPotential = Array(size + 1).fill(0);
  const matchedRowByColumn = Array(size + 1).fill(0);
  const predecessor = Array(size + 1).fill(0);

  for (let row = 1; row <= size; row += 1) {
    matchedRowByColumn[0] = row;
    const minimumReducedCost = Array(size + 1).fill(Infinity);
    const usedColumn = Array(size + 1).fill(false);
    let column = 0;
    do {
      usedColumn[column] = true;
      const activeRow = matchedRowByColumn[column];
      let delta = Infinity;
      let nextColumn = 0;
      for (let candidate = 1; candidate <= size; candidate += 1) {
        if (usedColumn[candidate]) continue;
        const edgeCost = costMatrix[activeRow - 1][candidate - 1];
        const reducedCost =
          edgeCost - rowPotential[activeRow] - columnPotential[candidate];
        if (reducedCost < minimumReducedCost[candidate]) {
          minimumReducedCost[candidate] = reducedCost;
          predecessor[candidate] = column;
        }
        if (minimumReducedCost[candidate] < delta) {
          delta = minimumReducedCost[candidate];
          nextColumn = candidate;
        }
      }
      if (!Number.isFinite(delta)) {
        return { assignment: [], cost: Infinity };
      }
      for (let candidate = 0; candidate <= size; candidate += 1) {
        if (usedColumn[candidate]) {
          rowPotential[matchedRowByColumn[candidate]] += delta;
          columnPotential[candidate] -= delta;
        } else {
          minimumReducedCost[candidate] -= delta;
        }
      }
      column = nextColumn;
    } while (matchedRowByColumn[column] !== 0);

    do {
      const previousColumn = predecessor[column];
      matchedRowByColumn[column] = matchedRowByColumn[previousColumn];
      column = previousColumn;
    } while (column !== 0);
  }

  const assignment = Array(size).fill(-1);
  for (let column = 1; column <= size; column += 1) {
    assignment[matchedRowByColumn[column] - 1] = column - 1;
  }
  const cost = assignment.reduce(
    (total, column, row) => total + costMatrix[row][column],
    0
  );
  return { assignment, cost };
}

function maxVectorDelta(before, after) {
  if (before.length === 0 || before.length !== after.length) return Infinity;
  const afterByRelationId = new Map(
    after.map((vector) => [vector.relationKey, vector])
  );
  return Math.max(
    ...before.map((vector) => {
      const next = afterByRelationId.get(vector.relationKey);
      if (!next) return Infinity;
      return Math.max(
        Math.abs(vector.left - next.left),
        Math.abs(vector.top - next.top)
      );
    })
  );
}

function pendingPhaseEvidence(artifacts) {
  return {
    status: 'pending',
    startedAt: null,
    completedAt: null,
    artifacts,
    pageErrors: [],
    consoleErrors: [],
    evidence: null,
    error: null
  };
}

async function runEvidencePhase({
  evidence,
  phase,
  pageErrors,
  consoleErrors,
  run,
  assertResult,
  collectFailureEvidence
}) {
  const phaseEvidence = evidence.phases[phase];
  const pageErrorStart = pageErrors.length;
  const consoleErrorStart = consoleErrors.length;
  phaseEvidence.status = 'running';
  phaseEvidence.startedAt = new Date().toISOString();
  let result = null;
  try {
    result = await run();
    assertResult(result);
    phaseEvidence.pageErrors = pageErrors.slice(pageErrorStart);
    phaseEvidence.consoleErrors = consoleErrors.slice(consoleErrorStart);
    if (
      phaseEvidence.pageErrors.length > 0 ||
      phaseEvidence.consoleErrors.length > 0
    ) {
      throw new Error(`${phase} browser phase emitted page or console errors.`);
    }
    phaseEvidence.status = 'passed';
    phaseEvidence.evidence = result;
    phaseEvidence.artifacts = result.artifacts ?? {};
    phaseEvidence.completedAt = new Date().toISOString();
    atomicWriteJson(evidencePath, evidence);
    return result;
  } catch (error) {
    if (result === null && collectFailureEvidence) {
      try {
        result = await collectFailureEvidence();
      } catch {
        // Preserve the phase's original failure when the page is unavailable.
      }
    }
    phaseEvidence.status = 'failed';
    phaseEvidence.evidence = result;
    phaseEvidence.artifacts = result?.artifacts ?? phaseEvidence.artifacts;
    phaseEvidence.pageErrors = pageErrors.slice(pageErrorStart);
    phaseEvidence.consoleErrors = consoleErrors.slice(consoleErrorStart);
    phaseEvidence.error = serializeError(error, phase);
    phaseEvidence.completedAt = new Date().toISOString();
    evidence.run.completedAt = phaseEvidence.completedAt;
    atomicWriteJson(evidencePath, evidence);
    throw error;
  }
}

function atomicWriteJson(path, value) {
  const temporaryPath = `${path}.${process.pid}.${Date.now()}.tmp`;
  try {
    writeFileSync(temporaryPath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
    renameSync(temporaryPath, path);
  } catch (error) {
    if (existsSync(temporaryPath)) unlinkSync(temporaryPath);
    throw error;
  }
}

async function atomicScreenshot(locator, path) {
  const temporaryPath = path.replace(
    /\.png$/u,
    `.temporary-${process.pid}-${Date.now()}.png`
  );
  try {
    await locator.screenshot({ path: temporaryPath });
    renameSync(temporaryPath, path);
  } catch (error) {
    if (existsSync(temporaryPath)) unlinkSync(temporaryPath);
    throw error;
  }
}

function serializeError(error, phase) {
  return {
    phase,
    name: error instanceof Error ? error.name : 'Error',
    message: error instanceof Error ? error.message : String(error),
    stack: error instanceof Error ? (error.stack ?? null) : null
  };
}

function assertStaticAcceptance(staticStyles) {
  const allContentBoxesAre280 = staticStyles.itemContentBoxes.every(
    (size) =>
      Math.abs((size?.width ?? 0) - 280) <= 0.5 &&
      Math.abs((size?.height ?? 0) - 280) <= 0.5
  );
  if (
    staticStyles.itemCount !== 12 ||
    !allContentBoxesAre280 ||
    !staticStyles.itemRuleInShadow ||
    staticStyles.itemRuleInDocumentHead ||
    staticStyles.popoverCount !== 12 ||
    staticStyles.visiblePopoverCount !== 12 ||
    staticStyles.pairs.length !== 12 ||
    staticStyles.matching.matrixShape[0] !== 12 ||
    staticStyles.matching.matrixShape[1] !== 12 ||
    !Number.isFinite(staticStyles.matching.optimalCostSquaredPixels) ||
    staticStyles.matching.ambiguityMarginSquaredPixels <
      minimumAssignmentMarginSquaredPixels ||
    !staticStyles.ownerScrollChanged ||
    !staticStyles.documentScrollUnchanged ||
    staticStyles.scrollAlignmentError > 3 ||
    staticStyles.resizeAlignmentError > 3
  ) {
    throw new Error(
      `Static Surface browser acceptance failed: ${JSON.stringify(staticStyles)}`
    );
  }
}

function assertRevealAcceptance(reveal) {
  const fixtureMountStayedAtEdge =
    reveal.fixtureMountOperations.length > 0 &&
    reveal.fixtureMountOperations.every(
      (operation) =>
        operation.visibleHeight <= 18 &&
        operation.visibleHeight < operation.targetHeight - 0.5 &&
        operation.documentBefore.top === operation.documentAfter.top &&
        operation.documentBefore.left === operation.documentAfter.left
    );
  if (
    !fixtureMountStayedAtEdge ||
    reveal.relations.length !== 1 ||
    !reveal.buttonVisible ||
    !reveal.popoverVisible ||
    !reveal.ownerScrollChanged ||
    !reveal.documentScrollUnchanged ||
    reveal.scrollAlignmentError > 3 ||
    reveal.resizeAlignmentError > 3
  ) {
    throw new Error(
      `Reveal Surface browser acceptance failed: ${JSON.stringify(reveal)}`
    );
  }
}

main().catch((error) => {
  process.stderr.write(`${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
