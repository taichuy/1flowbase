const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { createRequire } = require('node:module');
const { chromium } = createRequire(
  path.join(process.cwd(), 'web/package.json')
)('playwright');

(async () => {
  const output = path.join(
    process.cwd(),
    'tmp/test-governance/vditor-loading/loading-browser'
  );
  fs.mkdirSync(output, { recursive: true });
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH
  });
  try {
    const page = await browser.newPage();
    let release;
    const gate = new Promise((resolve) => {
      release = resolve;
    });
    const requested = [];
    page.on('request', (request) => requested.push(request.url()));
    await page.route('**/lute.min.js', async (route) => {
      await gate;
      await route.continue();
    });
    await page.goto(
      'http://127.0.0.1:3100/src/features/frontstage/_tests/browser-fixtures/rich-text/index.html',
      { waitUntil: 'domcontentloaded' }
    );
    const status = page.getByRole('status').filter({ hasText: '编辑器加载中' });
    await status.waitFor();
    await page.waitForFunction(() => {
      const frames = document
        .getElementById('root')
        .shadowRoot.querySelectorAll('iframe');
      return [...frames].some((frame) =>
        frame.contentDocument?.getElementById('vditorHljsThirdScript')
      );
    });
    assert.ok(requested.some((url) => url.endsWith('/lute.min.js')));
    assert.ok(requested.some((url) => url.endsWith('/index.min.js')));
    await page.screenshot({ path: path.join(output, 'loading-desktop.png') });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.screenshot({ path: path.join(output, 'loading-mobile.png') });
    release();
    await status.waitFor({ state: 'hidden' });
    const editor = page.frameLocator('iframe[title="Fixture editor"]');
    await editor.locator('.vditor-ir pre[contenteditable=true]').waitFor();
    await page.screenshot({ path: path.join(output, 'ready-mobile.png') });
    const resources = await editor.locator('body').evaluate(() =>
      performance
        .getEntriesByType('resource')
        .filter((entry) =>
          /lute.min.js|index.min.js|highlight.min.js|ant.js/.test(entry.name)
        )
        .map((entry) => ({
          name: entry.name,
          startTime: entry.startTime,
          duration: entry.duration,
          encodedBodySize: entry.encodedBodySize
        }))
    );
    await page.close();
    const failed = await browser.newPage();
    await failed.route('**/lute.min.js', (route) => route.abort('failed'));
    await failed.goto(
      'http://127.0.0.1:3100/src/features/frontstage/_tests/browser-fixtures/rich-text/index.html'
    );
    await failed
      .getByRole('alert')
      .filter({ hasText: '编辑器加载失败' })
      .waitFor();
    await failed
      .getByRole('alert')
      .filter({ hasText: '正文加载失败' })
      .waitFor();
    assert.equal(await failed.locator('div[role="status"]').count(), 0);
    fs.writeFileSync(
      path.join(output, 'result.json'),
      JSON.stringify({ passed: true, resources }, null, 2)
    );
    console.log(
      'PASS slow parser loading, parallel resources, desktop/mobile status, ready transition, parser failure'
    );
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
