const assert = require('node:assert/strict');
const path = require('node:path');
const { createRequire } = require('node:module');
const { chromium } = createRequire(
  path.join(process.cwd(), 'web/package.json')
)('playwright');
(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH
  });
  try {
    const page = await browser.newPage();
    await page.goto(
      'http://127.0.0.1:3100/src/features/frontstage/_tests/browser-fixtures/rich-text/index.html'
    );
    const frame = page.frameLocator('iframe[title="Fixture editor"]');
    const current = (mode) =>
      frame
        .locator(`[data-mode="${mode}"].vditor-menu--current`)
        .waitFor({ state: 'attached' });
    await current('ir');
    for (const mode of ['sv', 'wysiwyg']) {
      await frame.locator('[data-type="edit-mode"]').click();
      await frame.locator(`[data-mode="${mode}"]`).click();
      await page.waitForFunction(
        (mode) =>
          localStorage.getItem('1flowbase.rich-text.editor-mode') === mode,
        mode
      );
      await page.reload();
      await current(mode);
      await page
        .getByRole('button', { name: 'Toggle editor', exact: true })
        .click();
      await page
        .getByRole('button', { name: 'Toggle editor', exact: true })
        .click();
      await current(mode);
    }
    await frame.locator('.vditor-wysiwyg pre[contenteditable=true]').click();
    await page.keyboard.press('Control+Alt+8');
    await page.waitForFunction(
      () => localStorage.getItem('1flowbase.rich-text.editor-mode') === 'ir'
    );
    await page.reload();
    await current('ir');
    await page.evaluate(() =>
      localStorage.setItem('1flowbase.rich-text.editor-mode', 'invalid-mode')
    );
    await page.reload();
    await current('ir');
    assert.equal(await page.locator('[role="alert"]').count(), 0);
    console.log(
      'PASS defaults, mouse switches, reload/remount persistence, keyboard mode preference, invalid-value fallback'
    );
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
