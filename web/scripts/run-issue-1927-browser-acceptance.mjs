/* global document, Element, getComputedStyle, innerHeight, MutationObserver, PerformanceObserver, performance, process, requestAnimationFrame, scrollTo, window */
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import pageDebugCore from '../../scripts/node/page-debug/core.js';
import pageDebugAuth from '../../scripts/node/page-debug/auth.js';

const { buildChromiumLaunchOptions, loadPlaywright } = pageDebugCore;
const { loadRootCredentials, openTemporaryConsoleSession } = pageDebugAuth;
const scriptDirectory = dirname(fileURLToPath(import.meta.url));

const repoRoot = resolve(scriptDirectory, '../..');
const outputDir = join(
  repoRoot,
  'tmp/test-governance/issue-1927-browser'
);
mkdirSync(outputDir, { recursive: true });
const storageStatePath = join(outputDir, 'storage-state.json');
const targetUrl =
  'http://127.0.0.1:3100/demo/pages/01a047b1-4856-7c20-bac7-4dfd060b2161';
const blockIds = {
  staticMenu: '01a047b3-46e4-7013-be44-7c6cbed19cdd',
  collapsibleMenu: '01a047b3-4a81-79e0-8e71-b8a8b6189f49',
  nestedMenu: '01a047b3-15d8-7192-a400-e2fb2859699c'
};
const reducedMotion = process.env.REDUCED_MOTION === '1';
const measureGeometry = reducedMotion;

async function main() {
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
      viewport: { width: 1586, height: 1129 },
      reducedMotion: reducedMotion ? 'reduce' : 'no-preference'
    });
    const page = await context.newPage();
    const pageErrors = [];
    page.on('pageerror', (error) => pageErrors.push(error.message));
    await page.addInitScript(() => {
      window.__switchLatencyAudit = { events: [], longTasks: [] };
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          window.__switchLatencyAudit.longTasks.push({
            startTime: entry.startTime,
            duration: entry.duration
          });
        }
      }).observe({ type: 'longtask', buffered: true });
      try {
        new PerformanceObserver((list) => {
          for (const entry of list.getEntries()) {
            window.__switchLatencyAudit.events.push({
              name: entry.name,
              startTime: entry.startTime,
              duration: entry.duration,
              processingStart: entry.processingStart,
              processingEnd: entry.processingEnd,
              interactionId: entry.interactionId
            });
          }
        }).observe({ type: 'event', buffered: true, durationThreshold: 16 });
      } catch {
        // Event Timing is not available in every browser engine.
      }
    });

    await page.goto(targetUrl, { waitUntil: 'domcontentloaded' });
    await warmWholePage(page);
    for (const blockId of Object.values(blockIds)) {
      const targetBlock = page.locator(
        `[data-flowbase-frontstage-block-id="${blockId}"]`
      );
      await targetBlock.waitFor({ state: 'attached', timeout: 30000 });
      await targetBlock.scrollIntoViewIfNeeded();
      await page.waitForFunction(
        (id) =>
          document
            .querySelector(`[data-flowbase-frontstage-block-id="${id}"]`)
            ?.getAttribute('data-flowbase-frontstage-render-status') ===
          'ready',
        blockId,
        { timeout: 60000 }
      );
      await page.waitForTimeout(300);
    }
    await page.waitForTimeout(2500);

    const interactions = [];
    const staticBlock = page.locator(
      `[data-flowbase-frontstage-block-id="${blockIds.staticMenu}"]`
    );
    await staticBlock.scrollIntoViewIfNeeded();
    await waitForPreparationFrontier(page);
    const staticNavigation = staticBlock
      .getByText('Navigation One', { exact: true })
      .first();
    const staticIterations = reducedMotion ? 2 : 8;
    for (let index = 0; index < staticIterations; index += 1) {
      interactions.push(
        await measureInteraction(
          page,
          staticBlock,
          staticNavigation,
          `static-submenu-${index}`
        )
      );
      await page.waitForTimeout(250);
    }

    const collapsibleBlock = page.locator(
      `[data-flowbase-frontstage-block-id="${blockIds.collapsibleMenu}"]`
    );
    await collapsibleBlock.scrollIntoViewIfNeeded();
    await waitForPreparationFrontier(page);
    const collapseButton = collapsibleBlock.getByRole('button').first();
    const collapseIterations = reducedMotion ? 2 : 6;
    for (let index = 0; index < collapseIterations; index += 1) {
      interactions.push(
        await measureInteraction(
          page,
          collapsibleBlock,
          collapseButton,
          `inline-collapse-${index}`
        )
      );
      await page.waitForTimeout(250);
    }
    const collapsibleNavigation = collapsibleBlock
      .getByText('Navigation One', { exact: true })
      .first();
    const submenuIterations = reducedMotion ? 2 : 6;
    for (let index = 0; index < submenuIterations; index += 1) {
      interactions.push(
        await measureInteraction(
          page,
          collapsibleBlock,
          collapsibleNavigation,
          `collapsible-submenu-${index}`
        )
      );
      await page.waitForTimeout(250);
    }

    const nestedBlock = page.locator(
      `[data-flowbase-frontstage-block-id="${blockIds.nestedMenu}"]`
    );
    await nestedBlock.scrollIntoViewIfNeeded();
    await waitForPreparationFrontier(page);
    const nestedNavigation = nestedBlock.getByText('Navigation Two', {
      exact: true
    });
    const nestedIterations = reducedMotion ? 2 : 6;
    for (let index = 0; index < nestedIterations; index += 1) {
      interactions.push(
        await measureInteraction(
          page,
          nestedBlock,
          nestedNavigation,
          `nested-submenu-${index}`
        )
      );
      await page.waitForTimeout(250);
    }

    const audit = await page.evaluate(() => window.__switchLatencyAudit);
    const evidence = {
      targetUrl,
      blockIds,
      reducedMotion,
      interactions,
      eventSummary: summarizeEvents(audit.events, interactions),
      longTasksDuringInteractions: audit.longTasks.filter((task) =>
        interactions.some(
          (interaction) =>
            task.startTime < interaction.auditEnd &&
            task.startTime + task.duration > interaction.clickStart
        )
      ),
      pageErrors
    };
    evidence.acceptance = buildAcceptanceSummary(evidence);
    writeFileSync(
      join(
        outputDir,
        reducedMotion ? 'evidence-reduced-motion.json' : 'evidence.json'
      ),
      `${JSON.stringify(evidence, null, 2)}\n`
    );
    await page.screenshot({
      path: join(
        outputDir,
        reducedMotion ? 'page-reduced-motion.png' : 'page.png'
      ),
      fullPage: true
    });
    assertAcceptance(evidence.acceptance, reducedMotion);
    process.stdout.write(`${JSON.stringify(evidence, null, 2)}\n`);
    await context.close();
  } finally {
    await browser?.close();
    await session.dispose();
  }
}

async function scrollThroughPage(page) {
  const geometry = await page.evaluate(() => {
    const owner = document.querySelector(
      '[data-flowbase-frontstage-scroll-owner]'
    );
    return owner
      ? { clientHeight: owner.clientHeight, scrollHeight: owner.scrollHeight }
      : {
          clientHeight: innerHeight,
          scrollHeight: document.documentElement.scrollHeight
        };
  });
  const step = Math.max(240, Math.floor(geometry.clientHeight * 0.75));
  for (let top = 0; top <= geometry.scrollHeight; top += step) {
    await page.evaluate((nextTop) => {
      const owner = document.querySelector(
        '[data-flowbase-frontstage-scroll-owner]'
      );
      if (owner) owner.scrollTop = nextTop;
      else scrollTo(0, nextTop);
    }, top);
    await page.waitForTimeout(180);
  }
}

async function warmWholePage(page) {
  await scrollThroughPage(page);
  await waitForPreparationFrontier(page);
}

async function waitForPreparationFrontier(page) {
  let stablePasses = 0;
  let lastState;
  for (let pass = 0; pass < 12; pass += 1) {
    await page.waitForTimeout(500);
    const state = await page.evaluate(() => {
      const owner = document.querySelector(
        '[data-flowbase-frontstage-scroll-owner]'
      );
      const frames = [
        ...document.querySelectorAll('[data-flowbase-frontstage-block-id]')
      ];
      const statuses = frames.map((frame) => ({
        blockId: frame.getAttribute('data-flowbase-frontstage-block-id'),
        status: frame.getAttribute('data-flowbase-frontstage-render-status')
      }));
      return {
        scrollHeight:
          owner?.scrollHeight ?? document.documentElement.scrollHeight,
        active: statuses.filter(
          ({ status }) =>
            status !== null &&
            !['idle', 'ready', 'failed', 'disposed'].includes(status)
        )
      };
    });
    lastState = state;
    stablePasses = state.active.length === 0 ? stablePasses + 1 : 0;
    if (stablePasses >= 2) return;
  }
  throw new Error(
    `Frontstage page did not reach a stable preparation frontier: ${JSON.stringify(lastState)}`
  );
}

async function measureInteraction(page, block, target, label) {
  await target.waitFor({ state: 'visible', timeout: 15000 });
  await block.evaluate((frame, options) => {
    const { currentLabel, measureGeometry } = options;
    const audit = {
      label: currentLabel,
      armedAt: performance.now(),
      clickStart: null,
      mutations: [],
      frames: [],
      geometryFrames: [],
      intrinsicCommits: []
    };
    window.__activeSwitchInteraction = audit;
    const recordClick = (event) => {
      if (window.__activeSwitchInteraction !== audit || audit.clickStart) {
        return;
      }
      audit.clickStart = performance.now();
      audit.eventTimeStamp = event.timeStamp;
      requestAnimationFrame((firstFrame) => {
        audit.frames.push(firstFrame);
        requestAnimationFrame((secondFrame) => audit.frames.push(secondFrame));
      });
      const geometryStartedAt = performance.now();
      const sampleGeometry = (now) => {
        const geometryRoot =
          frame.querySelector('[data-flowbase-native-trusted-block-root]')
            ?.shadowRoot ?? frame;
        audit.geometryFrames.push({
          now,
          menus: Array.from(geometryRoot.querySelectorAll('ul')).map((menu) => {
            const rect = menu.getBoundingClientRect();
            return [
              Math.round(rect.width * 10) / 10,
              Math.round(rect.height * 10) / 10,
              getComputedStyle(menu).display
            ];
          })
        });
        if (now - geometryStartedAt < 400) requestAnimationFrame(sampleGeometry);
      };
      if (measureGeometry) requestAnimationFrame(sampleGeometry);
    };
    frame.addEventListener('pointerdown', recordClick, {
      capture: true,
      once: true
    });
    const roots = [frame];
    const runtimeHost = frame.querySelector(
      '[data-flowbase-native-trusted-block-root]'
    );
    if (runtimeHost?.shadowRoot) roots.push(runtimeHost.shadowRoot);
    const observer = new MutationObserver((records) => {
      const now = performance.now();
      for (const record of records) {
        audit.mutations.push({
          now,
          type: record.type,
          attributeName: record.attributeName,
          target:
            record.target instanceof Element
              ? `${record.target.tagName}.${String(record.target.className)}`.slice(
                  0,
                  240
                )
              : record.target.nodeName
        });
      }
    });
    roots.forEach((root) =>
      observer.observe(root, {
        subtree: true,
        attributes: true,
        attributeFilter: ['class', 'style', 'aria-expanded'],
        childList: true,
        characterData: true
      })
    );
    const documentObserver = new MutationObserver((records) => {
      for (const record of records) {
        audit.intrinsicCommits.push({
          now: performance.now(),
          blockId: record.target.getAttribute(
            'data-flowbase-frontstage-block-id'
          ),
          height: record.target.getAttribute(
            'data-flowbase-frontstage-intrinsic-height'
          )
        });
      }
    });
    documentObserver.observe(document.body, {
      subtree: true,
      attributes: true,
      attributeFilter: ['data-flowbase-frontstage-intrinsic-height']
    });
    audit.stop = () => {
      observer.disconnect();
      documentObserver.disconnect();
      delete audit.stop;
    };
  }, { currentLabel: label, measureGeometry });

  await target.click();
  await page.waitForTimeout(700);
  return block.evaluate(() => {
    const audit = window.__activeSwitchInteraction;
    audit.auditEnd = performance.now();
    audit.stop?.();
    const clickStart = audit.clickStart ?? audit.armedAt;
    let lastGeometryChangeAt = null;
    let previousGeometry = null;
    for (const sample of audit.geometryFrames) {
      const geometry = JSON.stringify(sample.menus);
      if (previousGeometry !== null && geometry !== previousGeometry) {
        lastGeometryChangeAt = sample.now - clickStart;
      }
      previousGeometry = geometry;
    }
    return {
      ...audit,
      firstMutationDelay:
        audit.mutations.length > 0 ? audit.mutations[0].now - clickStart : null,
      lastMutationDelay:
        audit.mutations.length > 0
          ? audit.mutations.at(-1).now - clickStart
          : null,
      firstFrameDelay:
        audit.frames.length > 0 ? audit.frames[0] - clickStart : null,
      secondFrameDelay:
        audit.frames.length > 1 ? audit.frames[1] - clickStart : null,
      lastGeometryChangeAt,
      mutationCount: audit.mutations.length
    };
  });
}

function summarizeEvents(events, interactions) {
  return interactions.map((interaction) => {
    const matching = events.filter(
      (event) =>
        event.startTime >= interaction.clickStart - 20 &&
        event.startTime <= interaction.auditEnd
    );
    return {
      label: interaction.label,
      events: matching,
      maxDuration: matching.reduce(
        (maximum, event) => Math.max(maximum, event.duration),
        0
      ),
      maxProcessingDelay: matching.reduce(
        (maximum, event) =>
          Math.max(maximum, event.processingStart - event.startTime),
        0
      )
    };
  });
}

function buildAcceptanceSummary(evidence) {
  const targetIds = new Set(Object.values(evidence.blockIds));
  const submenuInteractions = evidence.interactions.filter(
    (interaction) => !interaction.label.startsWith('inline-collapse-')
  );
  const firstMutationDelays = submenuInteractions
    .map((interaction) => interaction.firstMutationDelay)
    .filter((value) => typeof value === 'number' && value >= 0);
  const inlineCollapseFirstMutationDelays = evidence.interactions
    .filter((interaction) => interaction.label.startsWith('inline-collapse-'))
    .map((interaction) => interaction.firstMutationDelay)
    .filter((value) => typeof value === 'number' && value >= 0);
  const geometryChanges = evidence.interactions.map((interaction) => {
    let previous = null;
    let count = 0;
    for (const frame of interaction.geometryFrames) {
      const signature = JSON.stringify(
        frame.menus.map(([width, height]) => [width, height])
      );
      if (signature !== previous) count += 1;
      previous = signature;
    }
    return count;
  });
  const interactionsWithLongTask = evidence.interactions.filter(
    (interaction) =>
      evidence.longTasksDuringInteractions.some(
        (task) =>
          task.startTime < interaction.auditEnd &&
          task.startTime + task.duration > interaction.clickStart
      )
  ).length;
  return {
    firstMutationP95: percentile95(firstMutationDelays),
    inlineCollapseFirstMutationP95: percentile95(
      inlineCollapseFirstMutationDelays
    ),
    eventDurationP95: percentile95(
      evidence.eventSummary.map((summary) => summary.maxDuration)
    ),
    processingDelayP95: percentile95(
      evidence.eventSummary.map((summary) => summary.maxProcessingDelay)
    ),
    motionCleanupP95: percentile95(
      evidence.interactions.map((interaction) => interaction.lastMutationDelay)
    ),
    targetIntrinsicCommits: evidence.interactions
      .flatMap((interaction) => interaction.intrinsicCommits)
      .filter((commit) => targetIds.has(commit.blockId)).length,
    motionClassMutations: evidence.interactions
      .flatMap((interaction) => interaction.mutations)
      .filter(
        (mutation) =>
          mutation.target.includes('motion-collapse') ||
          mutation.target.includes('wave-motion')
      ).length,
    longTasks: evidence.longTasksDuringInteractions.length,
    longTaskInteractionRate:
      interactionsWithLongTask / evidence.interactions.length,
    maxGeometryStates: Math.max(...geometryChanges),
    pageErrors: evidence.pageErrors.length
  };
}

function assertAcceptance(summary, isReducedMotion) {
  const failures = [];
  if (summary.pageErrors !== 0) failures.push('pageErrors');
  if (summary.targetIntrinsicCommits !== 0)
    failures.push('targetIntrinsicCommits');
  if (summary.motionClassMutations !== 0)
    failures.push('motionClassMutations');
  if (isReducedMotion) {
    if (summary.maxGeometryStates > 2) failures.push('maxGeometryStates');
  } else {
    if (summary.firstMutationP95 > 50) failures.push('firstMutationP95');
    if (summary.eventDurationP95 > 100) failures.push('eventDurationP95');
    if (summary.processingDelayP95 > 50)
      failures.push('processingDelayP95');
    if (summary.motionCleanupP95 > 160) failures.push('motionCleanupP95');
  }
  if (failures.length > 0) {
    throw new Error(
      `Issue #1927 browser acceptance failed: ${JSON.stringify({
        failures,
        summary
      })}`
    );
  }
}

function percentile95(values) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.ceil(sorted.length * 0.95) - 1];
}

main().catch((error) => {
  process.stderr.write(`${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
