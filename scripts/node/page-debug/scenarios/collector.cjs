// Authenticated runtime evidence. The page-debug session owner is always disposed.
const fs = require('node:fs/promises');
const path = require('node:path');
const { createRequire } = require('node:module');
const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const { loadRootCredentials, openTemporaryConsoleSession } = require('../auth');

async function main() {
  const repoRoot = path.resolve(__dirname, '../../../..');
  const webBaseUrl = process.env.COLLECTOR_WEB_BASE_URL || 'http://127.0.0.1:3100';
  const apiBaseUrl = process.env.COLLECTOR_API_BASE_URL || 'http://127.0.0.1:7800';
  const applicationId = process.env.COLLECTOR_APPLICATION_ID || '01a11699-d833-7373-b825-91d9916896b8';
  const out = process.env.COLLECTOR_EVIDENCE_DIR || path.join(repoRoot, 'tmp/test-governance/agent-logs-native-collector/browser');
  const playwright = createRequire(path.join(repoRoot, 'web/package.json'))('playwright');
  await fs.mkdir(out, { recursive: true });
  const storageStatePath = path.join(out, 'temporary-storage-state.json');
  const credentials = loadRootCredentials({ repoRoot });
  const owner = await openTemporaryConsoleSession({ playwright, apiBaseUrl, ...credentials, storageStatePath });
  let browser;
  let cleanupProbe;
  const receipts = [];
  try {
    await fs.chmod(storageStatePath, 0o600);
    cleanupProbe = await playwright.request.newContext({ storageState: storageStatePath });
    browser = await playwright.chromium.launch({ executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH || '/usr/bin/google-chrome', headless: true });
    for (const [name, viewport] of [['desktop', { width: 1440, height: 1000 }], ['mobile', { width: 390, height: 844 }]]) {
      const context = await browser.newContext({ storageState: storageStatePath, viewport });
      await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: webBaseUrl });
      const page = await context.newPage();
      const errors = []; page.on('pageerror', error => errors.push(error.message));
      await page.goto(`${webBaseUrl}/applications/${applicationId}/collector`, { waitUntil: 'domcontentloaded' });
      const install = page.getByRole('button', { name: /安装采集器|Install collector/ });
      await install.waitFor({ state: 'visible', timeout: 30000 });
      const catalogResponse = await context.request.get(`${apiBaseUrl}/api/console/applications/catalog`);
      assert.equal(catalogResponse.status(), 200);
      const catalog = (await catalogResponse.json()).data;
      assert.deepEqual(catalog.collectors.map(item => item.collector_code), ['codex-logs-collector']);
      assert.equal(catalog.collectors[0].execution_target, 'client');
      await page.getByRole('tab', { name: 'Codex', exact: true }).click();
      await install.waitFor({ state: 'visible' });
      assert.equal(await page.getByText('Codex', { exact: true }).count() > 0, true);
      assert.equal(/node scripts\/node\/agent-logs-collector|已安装/.test(await page.locator('body').innerText()), false);
      await page.screenshot({ path: path.join(out, `${name}-catalog.png`), fullPage: true });
      await install.click();
      await page.getByText('macOS / Linux (Shell)', { exact: true }).waitFor();
      let text = await page.locator('body').innerText();
      assert.ok(text.includes('bash "$installer"')); assert.ok(text.includes(applicationId));
      assert.ok(text.includes('/api/logs/v1/events')); assert.ok(!text.includes('?api_key='));
      const shellCommand = await page.locator('.application-collector__command pre').innerText();
      await page.getByRole('button', { name: /复制命令|Copy command/ }).click();
      await page.waitForFunction(expected => navigator.clipboard.readText().then(value => value === expected), shellCommand);
      const overflow = await page.evaluate(() => ({ width: document.documentElement.clientWidth, scroll: document.documentElement.scrollWidth }));
      assert.ok(overflow.scroll <= overflow.width + 1, `overflow ${JSON.stringify(overflow)}`);
      await page.screenshot({ path: path.join(out, `${name}-shell.png`), fullPage: true });
      await page.getByText('Windows (PowerShell)', { exact: true }).click();
      text = await page.locator('body').innerText();
      assert.ok(text.includes('-Endpoint')); assert.ok(text.includes('-InstallationId'));
      await page.screenshot({ path: path.join(out, `${name}-powershell.png`), fullPage: true });
      const keys = page.getByRole('link', { name: /API Key/ }).first();
      assert.equal(await keys.getAttribute('href'), `/applications/${applicationId}/api`);
      await page.getByRole('button', { name: /返回|Back/ }).click();
      await install.waitFor();
      assert.deepEqual(errors, []);
      receipts.push({ name, viewport, overflow, errors, status: 'pass', filter: 'codex', clipboard: 'exact shell command', collector: catalog.collectors[0] });
      await context.close();
    }
    await fs.writeFile(path.join(out, 'receipt.json'), JSON.stringify({ source_sha: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repoRoot, encoding: 'utf8' }).trim(), application_id: applicationId, web_base_url: webBaseUrl, api_base_url: apiBaseUrl, receipts }, null, 2));
  } finally {
    if (browser) await browser.close();
    await owner.dispose();
    await fs.rm(storageStatePath, { force: true });
    if (cleanupProbe) {
      try {
        const status = (await cleanupProbe.get(`${apiBaseUrl}/api/console/session`)).status();
        await fs.writeFile(path.join(out, 'cleanup.json'), JSON.stringify({ revoked_session_status: status, storage_state_removed: true }, null, 2));
        assert.equal(status, 401);
      } finally { await cleanupProbe.dispose(); }
    }
  }
}
main().catch(error => { process.stderr.write(`${error.message}\n`); process.exitCode = 1; });
