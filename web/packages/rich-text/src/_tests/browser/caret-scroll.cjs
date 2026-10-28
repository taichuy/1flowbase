const assert = require('node:assert/strict');
const fs = require('node:fs');
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
  const results = [];
  try {
    for (const width of [1280, 390]) {
      for (const mode of ['sv', 'ir', 'wysiwyg']) {
        const page = await browser.newPage({
          viewport: { width, height: 900 }
        });
        await page.addInitScript(
          (mode) =>
            localStorage.setItem('1flowbase.rich-text.editor-mode', mode),
          mode
        );
        await page.goto(
          'http://127.0.0.1:3100/src/features/frontstage/_tests/browser-fixtures/rich-text/index.html?scroll'
        );
        const frame = page.frameLocator('iframe[title="Fixture editor"]');
        const editor = frame.locator(
          mode === 'sv'
            ? '.vditor-sv'
            : `.vditor-${mode} pre[contenteditable=true]`
        );
        await editor.click();
        await page.keyboard.press('Control+End');
        const before = await editor.evaluate((el) => el.scrollTop);
        for (let index = 0; index < 12; index++) {
          await page.keyboard.press('Enter');
          await editor.evaluate(
            () =>
              new Promise((resolve) =>
                requestAnimationFrame(() => requestAnimationFrame(resolve))
              )
          );
        }
        const geometry = await editor.evaluate((el) => {
          const selection = getSelection();
          const range = selection.getRangeAt(0);
          let rect = range.getBoundingClientRect();
          if (!rect.height) {
            const node = selection.anchorNode;
            const child =
              node.nodeType === 1
                ? (node.childNodes[selection.anchorOffset] ??
                  node.childNodes[selection.anchorOffset - 1])
                : node;
            const element =
              child?.nodeType === 1 ? child : child?.parentElement;
            rect = (element ?? el).getBoundingClientRect();
          }
          const bounds = el.getBoundingClientRect();
          return {
            scrollTop: el.scrollTop,
            top: bounds.top,
            bottom: bounds.bottom,
            caretTop: rect.top,
            caretBottom: rect.bottom,
            selectionInside: el.contains(selection.anchorNode)
          };
        });
        assert.ok(geometry.selectionInside);
        assert.ok(
          geometry.scrollTop > before,
          `${mode}/${width}: scroll did not advance`
        );
        assert.ok(
          geometry.caretTop >= geometry.top &&
            geometry.caretBottom <= geometry.bottom,
          JSON.stringify({ mode, width, geometry })
        );
        results.push({ mode, width, before, ...geometry });
        await page.close();
      }
    }
    fs.mkdirSync('tmp/test-governance/vditor-caret-scroll', {
      recursive: true
    });
    fs.writeFileSync(
      'tmp/test-governance/vditor-caret-scroll/browser.json',
      JSON.stringify(results, null, 2)
    );
    console.log(
      'PASS Enter follows caret in sv/ir/wysiwyg at desktop and mobile widths'
    );
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
