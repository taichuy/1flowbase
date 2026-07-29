import { mkdir, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const require = createRequire(import.meta.url);
const {
  createProbeUrl,
  resolveStyleBoundaryFrontendHost
} = require('../../scripts/node/check-style-boundary/core.js');

const webRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = resolve(webRoot, '..');
const output = resolve(
  repositoryRoot,
  'tmp/test-governance/issue-1488-settings-i18n-browser'
);
const sceneId = 'page.settings-i18n.desktop';
const expectedRequest = {
  search: 'Settings',
  module: '@1flowbase/common',
  locale: 'zh_Hans',
  origin: 'official_override',
  offset: '0',
  limit: '20'
};

await mkdir(output, { recursive: true });
const browser = await chromium.launch({ channel: 'chrome', headless: true });
let frontendHost = {
  baseUrl: '',
  stop: async () => {}
};

try {
  frontendHost = await resolveStyleBoundaryFrontendHost(
    browser,
    repositoryRoot,
    sceneId,
    process.env
  );
  const page = await browser.newPage({
    viewport: { width: 1280, height: 800 }
  });

  try {
    await page.goto(createProbeUrl(frontendHost.baseUrl, sceneId), {
      waitUntil: 'domcontentloaded'
    });
    await page.waitForFunction(() => window.__STYLE_BOUNDARY__?.ready === true);
    await page
      .locator('[data-testid="i18n-catalog-page"][data-ready="true"]')
      .waitFor();
    await page.getByText('系统设置', { exact: true }).first().waitFor();

    await page.getByTestId('i18n-catalog-search').fill(expectedRequest.search);
    await page
      .getByTestId('i18n-catalog-module-filter')
      .fill(expectedRequest.module);
    await chooseSelectOption(page, 'i18n-catalog-locale-filter', 0);
    await chooseSelectOption(page, 'i18n-catalog-origin-filter', 1);
    await page.getByTestId('i18n-catalog-apply-filters').click();

    await page.waitForFunction((expected) => {
      const request = window.__STYLE_BOUNDARY_I18N_CATALOG_REQUESTS__?.at(-1);
      return (
        request !== undefined &&
        Object.entries(expected).every(([key, value]) => request[key] === value)
      );
    }, expectedRequest);
    await page.getByText('系统设置', { exact: true }).first().waitFor();
    const requests = await page.evaluate(
      () => window.__STYLE_BOUNDARY_I18N_CATALOG_REQUESTS__ ?? []
    );
    const actualRequest = requests.at(-1);

    if (!actualRequest) {
      throw new Error('The browser fixture captured no i18n catalog request.');
    }
    for (const [key, value] of Object.entries(expectedRequest)) {
      if (actualRequest[key] !== value) {
        throw new Error(
          `Unexpected i18n catalog request ${key}: expected ${value}, received ${String(actualRequest[key])}`
        );
      }
    }

    const screenshotPath = resolve(output, 'settings-i18n-filter.png');
    await page.screenshot({ path: screenshotPath, fullPage: true });
    const evidence = {
      ok: true,
      sceneId,
      baseUrl: frontendHost.baseUrl,
      request: actualRequest,
      renderedEntry: '系统设置',
      screenshotPath
    };
    await writeFile(
      resolve(output, 'evidence.json'),
      `${JSON.stringify(evidence, null, 2)}\n`,
      'utf8'
    );
    process.stdout.write(`${JSON.stringify(evidence, null, 2)}\n`);
  } finally {
    await page.close();
  }
} finally {
  await frontendHost.stop();
  await browser.close();
}

async function chooseSelectOption(page, testId, optionIndex) {
  const select = page.getByTestId(testId);
  const combobox = select.getByRole('combobox');

  await select.locator('.ant-select-selector').click();
  await page
    .locator('.ant-select-dropdown:visible .ant-select-item-option')
    .nth(optionIndex)
    .click();
  await page.waitForFunction(
    (id) =>
      document
        .querySelector(`[data-testid="${id}"] input[role="combobox"]`)
        ?.getAttribute('aria-expanded') === 'false',
    testId
  );
  await page
    .locator('.ant-select-dropdown:visible')
    .waitFor({ state: 'hidden' });
  await combobox.waitFor({ state: 'attached' });
}
