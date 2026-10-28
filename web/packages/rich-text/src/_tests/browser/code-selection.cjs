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
    'tmp/test-governance/vditor-selection/browser'
  );
  fs.mkdirSync(output, { recursive: true });
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH
  });
  try {
    const page = await browser.newPage({
      viewport: { width: 1280, height: 1000 }
    });
    await page
      .context()
      .grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.goto(
      'http://127.0.0.1:3100/src/features/frontstage/_tests/browser-fixtures/rich-text/index.html'
    );
    const frame = page.frameLocator('iframe[title="Fixture editor"]');
    const source = frame.locator(
      '.vditor-ir__marker--pre code.language-javascript'
    );
    const preview = frame.locator(
      '.vditor-ir__preview code.language-javascript'
    );
    const results = [];
    for (const backwards of [false, true]) {
      if (backwards) await page.reload();
      await page
        .getByRole('button', { name: 'Load sample', exact: true })
        .click();
      await preview.locator('.hljs-keyword').waitFor();
      await frame.locator('body').evaluate(() => {
        window.selectionBeforeClick = '';
        document.addEventListener(
          'mouseup',
          () => {
            window.selectionBeforeClick = getSelection().toString();
          },
          { capture: true, once: true }
        );
      });
      const rect = await preview.locator('.hljs-keyword').boundingBox();
      const left = { x: rect.x + 8, y: rect.y + rect.height / 2 };
      const right = { x: rect.x + 100, y: left.y };
      const [start, end] = backwards ? [right, left] : [left, right];
      await page.mouse.move(start.x, start.y);
      await page.mouse.down();
      await page.mouse.move(end.x, end.y, { steps: 12 });
      await page.mouse.up();
      const expected = await frame
        .locator('body')
        .evaluate(() => window.selectionBeforeClick);
      assert.ok(expected.length > 3, 'drag must select actual code text');
      const selected = () =>
        frame.locator('body').evaluate(() => {
          const s = getSelection();
          return {
            text: s.toString(),
            anchor: s.anchorOffset,
            focus: s.focusOffset
          };
        });
      const before = await selected();
      assert.equal(before.text, expected);
      assert.equal(before.anchor > before.focus, backwards);
      const content = await source.textContent();
      await page.keyboard.press('Control+c');
      assert.equal(
        await page.evaluate(() => navigator.clipboard.readText()),
        expected
      );
      assert.deepEqual(await selected(), before);
      assert.equal(await source.textContent(), content);
      // A restored selection must still be editable, and remain part of native undo.
      await page.keyboard.insertText('replacement');
      assert.ok((await source.textContent()).includes('replacement'));
      await page.waitForFunction(() =>
        document
          .getElementById('root')
          .shadowRoot.querySelector('output')
          .textContent.includes('replacement')
      );
      await page.keyboard.press('Control+z');
      await page.waitForFunction((expected) => {
        const doc = document
          .getElementById('root')
          .shadowRoot.querySelector('iframe').contentDocument;
        return (
          doc.querySelector('.vditor-ir__marker--pre code.language-javascript')
            ?.textContent === expected
        );
      }, content);
      results.push({ backwards, expected, selection: before });
    }
    // Repeat inside an already expanded, multiline source block.
    await preview.click();
    await page.keyboard.press('Control+a');
    await page.keyboard.insertText('const first = 1;');
    await page.keyboard.press('Enter');
    await page.keyboard.insertText('const second = 2;');
    await page.keyboard.press('Enter');
    await page.keyboard.insertText('const third = 3;');
    await page.waitForFunction(() =>
      document
        .getElementById('root')
        .shadowRoot.querySelector('output')
        .textContent.includes('const third = 3;')
    );
    await source.evaluate((code) => {
      const r = document.createRange();
      r.selectNodeContents(code);
      const s = getSelection();
      s.removeAllRanges();
      s.addRange(r);
    });
    const multi = await frame
      .locator('body')
      .evaluate(() => getSelection().toString());
    assert.ok(
      multi.includes('const first = 1;\nconst second = 2;\nconst third = 3;')
    );
    await page.keyboard.press('Control+c');
    assert.equal(
      await page.evaluate(() => navigator.clipboard.readText()),
      multi.trim()
    );
    assert.equal(
      await frame.locator('body').evaluate(() => getSelection().toString()),
      multi
    );
    await page.screenshot({ path: path.join(output, 'selection.png') });
    fs.writeFileSync(
      path.join(output, 'result.json'),
      JSON.stringify({ passed: true, results }, null, 2)
    );
    console.log(
      'PASS forward/backward preview drag, Ctrl+C preserves selection/text, replacement and undo, multiline source copy'
    );
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
