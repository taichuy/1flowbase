const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { createRequire } = require('node:module');
const playwright = createRequire(path.join(process.cwd(), 'web/package.json'))(
  'playwright'
);

(async () => {
  const output = path.join(
    process.cwd(),
    'tmp/test-governance/wiki-editor-fix/browser'
  );
  fs.mkdirSync(output, { recursive: true });
  const browser = await playwright.chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH
  });
  const errors = [],
    failedRequests = [];
  try {
    const page = await browser.newPage({
      viewport: { width: 1280, height: 900 }
    });
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('response', (response) => {
      if (response.status() >= 400)
        failedRequests.push([response.status(), response.url()]);
    });
    const fixture =
      process.env.WIKI_VERIFY_FIXTURE_PATH ||
      '/src/features/frontstage/_tests/browser-fixtures/rich-text/index.html';
    await page.goto(
      `${process.env.WIKI_VERIFY_WEB_ORIGIN || 'http://127.0.0.1:3100'}${fixture}`
    );
    const editorFrame = page.frameLocator('iframe[title="Fixture editor"]');
    const editable = editorFrame.locator(
      '.vditor-ir pre[contenteditable=true]'
    );
    await editable.waitFor();
    await editorFrame
      .locator('#vditor-icon-bold')
      .waitFor({ state: 'attached' });
    assert.ok(
      await editorFrame
        .locator('[data-type="bold"] use')
        .evaluate((icon) => icon.getBBox().width > 0)
    );
    await editable.click();
    await editorFrame.locator('[data-type="fullscreen"]').click();
    await page.waitForFunction(() => Boolean(document.fullscreenElement));
    await editorFrame.locator('[data-type="fullscreen"]').click();
    await page.waitForFunction(() => !document.fullscreenElement);
    await editable.click();
    await page.keyboard.type('```', { delay: 50 });
    await editorFrame.locator('.vditor-hint:visible button').first().waitFor();
    await page.keyboard.type('js', { delay: 50 });
    await editorFrame
      .getByRole('button', { name: 'js', exact: true })
      .waitFor();
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.keyboard.type('const answer = 42;', { delay: 30 });
    await page.waitForFunction(() =>
      document
        .getElementById('root')
        .shadowRoot.querySelector('output')
        .textContent.includes('const answer = 42;')
    );
    const markdown = await page.getByTestId('markdown').textContent();
    assert.match(markdown, /```js\s+const answer = 42;/);
    await page.keyboard.press('Control+z');
    await page.waitForFunction(
      (expected) =>
        document.getElementById('root').shadowRoot.querySelector('output')
          .textContent !== expected,
      markdown
    );
    await page.keyboard.press('Control+y');
    await page.waitForFunction(
      (expected) =>
        document.getElementById('root').shadowRoot.querySelector('output')
          .textContent === expected,
      markdown
    );
    await page
      .getByRole('button', { name: 'Load sample', exact: true })
      .click();
    await editorFrame.locator('.katex').first().waitFor();
    await editorFrame
      .locator('[data-type="code-block"] svg[id^="mermaid"]')
      .waitFor({ timeout: 20000 });
    await page.getByRole('button', { name: 'Save', exact: true }).click();
    const reader = page.frameLocator('iframe[title="Fixture reader"]');
    await reader.locator('h1').waitFor();
    assert.equal(await reader.locator('h1').innerText(), '文档');
    await reader.locator('code.hljs .hljs-keyword').first().waitFor();
    await reader.locator('.katex').first().waitFor();
    await reader.locator('svg[id^="mermaid"]').waitFor({ timeout: 20000 });
    assert.equal(await reader.locator('table').count(), 1);
    await page.screenshot({
      path: path.join(output, 'editor-reader.png'),
      fullPage: true
    });
    await editable.locator('h1').click();
    await page.keyboard.press('Control+a');
    await page.keyboard.press('Backspace');
    await page.keyboard.type('```ts', { delay: 60 });
    await editorFrame
      .getByRole('button', { name: 'ts', exact: true })
      .waitFor();
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.keyboard.type('let value: number = 1;', { delay: 30 });
    await page.waitForFunction(() =>
      document
        .getElementById('root')
        .shadowRoot.querySelector('output')
        .textContent.includes('let value: number = 1;')
    );
    assert.match(
      await page.getByTestId('markdown').textContent(),
      /```ts\s+let value: number = 1;/
    );
    await page
      .getByRole('button', { name: 'Load sample', exact: true })
      .click();
    await editorFrame.locator('.katex').first().waitFor();
    await page
      .getByRole('button', { name: 'Toggle editor', exact: true })
      .click();
    await page
      .getByRole('button', { name: 'Toggle editor', exact: true })
      .click();
    await editorFrame.locator('.katex').first().waitFor();
    await editorFrame.locator('[data-type="edit-mode"]').click();
    await editorFrame.locator('[data-mode="wysiwyg"]').click();
    await editorFrame
      .locator('.vditor-wysiwyg pre[contenteditable=true]')
      .waitFor();
    assert.deepEqual(errors, []);
    assert.deepEqual(failedRequests, []);
    fs.rmSync(path.join(output, 'failure.json'), { force: true });
    fs.writeFileSync(
      path.join(output, 'result.json'),
      JSON.stringify(
        { passed: true, markdown, errors, failedRequests },
        null,
        2
      )
    );
    console.log(
      'PASS fence completion, language selection, code/math/Mermaid/table preview, remount, wysiwyg switch'
    );
  } catch (error) {
    fs.writeFileSync(
      path.join(output, 'failure.json'),
      JSON.stringify({ error: String(error), errors, failedRequests }, null, 2)
    );
    throw error;
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
