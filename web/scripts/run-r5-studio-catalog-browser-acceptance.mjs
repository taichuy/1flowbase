import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = resolve(webRoot, '..');
const output = resolve(
  repositoryRoot,
  'tmp/test-governance/root-1466-r5-final/browser'
);
const base = process.env.NATIVE_FIXTURE_BASE_URL ?? 'http://127.0.0.1:4175';
const executablePath =
  process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH ?? '/usr/bin/google-chrome';

await mkdir(output, { recursive: true });
const browser = await chromium.launch({ executablePath, headless: true });
const results = [];

try {
  for (const fixture of [
    { name: 'desktop', viewport: { width: 1440, height: 1000 } },
    { name: 'mobile-390', viewport: { width: 390, height: 844 } }
  ]) {
    results.push(await verifyFixture(browser, fixture));
  }
} finally {
  await browser.close();
}

const evidence = { ok: true, base, executablePath, results };
await writeFile(
  resolve(output, 'evidence.json'),
  `${JSON.stringify(evidence, null, 2)}\n`,
  'utf8'
);
console.log(JSON.stringify(evidence, null, 2));

async function verifyFixture(browserInstance, fixture) {
  const context = await browserInstance.newContext({
    viewport: fixture.viewport
  });
  const page = await context.newPage();
  const consoleErrors = [];
  const externalRequests = [];
  const httpErrors = [];
  const failedRequests = [];
  page.on('console', (message) => {
    if (message.type() === 'error') {
      consoleErrors.push({
        text: message.text(),
        location: message.location()
      });
    }
  });
  page.on('request', (request) => {
    if (new URL(request.url()).origin !== base) {
      externalRequests.push(request.url());
    }
  });
  page.on('response', (response) => {
    if (response.status() >= 400) {
      httpErrors.push({ url: response.url(), status: response.status() });
    }
  });
  page.on('requestfailed', (request) => {
    failedRequests.push({
      url: request.url(),
      errorText: request.failure()?.errorText ?? 'unknown'
    });
  });

  try {
    await page.goto(`${base}/r5-studio-catalog-fixture.html`, {
      waitUntil: 'networkidle'
    });
    const stats = page.locator('[data-testid=r5-studio-catalog-stats]');
    await stats.waitFor({ state: 'attached' });
    await page.getByText('Fixture runtime diagnostic').waitFor();
    await page
      .getByText('[api/succeeded] GET /api/console/example 12ms')
      .waitFor();
    await page.getByText('browser render 0').waitFor();
    await page.getByRole('button', { name: 'Emit runtime log' }).click();
    await page.getByText('browser clicked {"count": 0}').waitFor();
    await page.getByText('browser render 1').waitFor();

    const previewBox = await page
      .locator('[data-testid=js-block-preview-pane]')
      .boundingBox();
    const consoleBox = await page
      .locator('[data-testid=js-block-console-pane]')
      .boundingBox();
    if (!previewBox || !consoleBox || consoleBox.y <= previewBox.y) {
      throw new Error(`${fixture.name} Preview/Console order is invalid.`);
    }
    if (
      (await page.locator('.frontstage-jsx-studio__problems').count()) !== 0
    ) {
      throw new Error(
        `${fixture.name} still renders the retired editor footer.`
      );
    }

    await page.getByRole('button', { name: 'Compile current source' }).click();
    await waitForAttribute(page, 'data-compiler-status', 'passed');
    assertAttribute(
      fixture.name,
      'approved policy errors',
      await stats.getAttribute('data-policy-errors'),
      '0'
    );

    await page.getByRole('button', { name: 'Denied import' }).click();
    await waitForAttribute(page, 'data-policy-errors', '1');
    assertAttribute(
      fixture.name,
      'denied marker line',
      await stats.getAttribute('data-marker-line'),
      '1'
    );
    assertAttribute(
      fixture.name,
      'denied marker column',
      await stats.getAttribute('data-marker-column'),
      '1'
    );
    await page.getByRole('button', { name: 'Compile current source' }).click();
    await waitForAttribute(page, 'data-compiler-status', 'failed');
    await page.getByText(/Import source 'dayjs' is not allowed\./u).waitFor();

    await page.getByRole('button', { name: 'Add Block' }).click();
    const drawer = page.getByRole('dialog', { name: '新增区块' });
    await drawer.waitFor();
    const drawerBox = await drawer.boundingBox();
    if (!drawerBox || drawerBox.width > fixture.viewport.width + 1) {
      throw new Error(
        `${fixture.name} Catalog Drawer exceeds viewport: ${JSON.stringify(drawerBox)}`
      );
    }
    await page.screenshot({
      path: resolve(output, `r5-${fixture.name}.png`),
      fullPage: true
    });
    const reportRow = drawer
      .locator('.ant-list-item')
      .filter({ hasText: 'Third-party report' });
    await reportRow.getByRole('button', { name: '选择' }).click();
    await waitForAttribute(
      page,
      'data-selected-entry',
      'third-party-installation:report-block'
    );
    assertAttribute(
      fixture.name,
      'selected Catalog template',
      await stats.getAttribute('data-selected-template'),
      'selected'
    );

    assertNetworkAndConsole(
      fixture.name,
      externalRequests,
      httpErrors,
      failedRequests,
      consoleErrors
    );
    return {
      fixture: fixture.name,
      compilerApproved: 'passed',
      compilerDenied: 'failed',
      deniedLocation: { line: 1, column: 1 },
      selectedEntry: await stats.getAttribute('data-selected-entry'),
      selectedTemplate: await stats.getAttribute('data-selected-template'),
      runtimeConsoleCaptured: true,
      drawerWidth: drawerBox.width,
      externalRequests: externalRequests.length,
      httpErrors,
      failedRequests,
      consoleErrors
    };
  } finally {
    await context.close();
  }
}

async function waitForAttribute(page, name, value) {
  await page.waitForFunction(
    ({ selector, attribute, expected }) =>
      document.querySelector(selector)?.getAttribute(attribute) === expected,
    {
      selector: '[data-testid=r5-studio-catalog-stats]',
      attribute: name,
      expected: value
    }
  );
}

function assertAttribute(fixture, label, actual, expected) {
  if (actual !== expected) {
    throw new Error(
      `${fixture} ${label}: expected ${expected}, received ${String(actual)}`
    );
  }
}

function assertNetworkAndConsole(
  name,
  externalRequests,
  httpErrors,
  failedRequests,
  consoleErrors
) {
  if (externalRequests.length > 0) {
    throw new Error(
      `${name} external requests: ${externalRequests.join(', ')}`
    );
  }
  if (httpErrors.length > 0) {
    throw new Error(
      `${name} HTTP errors: ${httpErrors
        .map(({ status, url }) => `${status} ${url}`)
        .join(' | ')}`
    );
  }
  if (failedRequests.length > 0) {
    throw new Error(
      `${name} failed requests: ${failedRequests
        .map(({ errorText, url }) => `${errorText} ${url}`)
        .join(' | ')}`
    );
  }
  if (consoleErrors.length > 0) {
    throw new Error(
      `${name} console errors: ${consoleErrors
        .map(({ text }) => text)
        .join(' | ')}`
    );
  }
}
