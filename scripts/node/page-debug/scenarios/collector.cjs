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
  const out = process.env.COLLECTOR_EVIDENCE_DIR || path.join(repoRoot, 'tmp/test-governance/agent-logs-platform-distribution/browser');
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
    for (const [name, language, viewport] of [['desktop-zh', 'zh', { width: 1440, height: 1000 }], ['mobile-zh', 'zh', { width: 390, height: 844 }], ['desktop-en', 'en', { width: 1440, height: 1000 }], ['mobile-en', 'en', { width: 390, height: 844 }]]) {
      const context = await browser.newContext({ storageState: storageStatePath, viewport });
      await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: webBaseUrl });
      const page = await context.newPage();
      const errors = []; page.on('pageerror', error => errors.push(error.message));
      await page.goto(`${webBaseUrl}/applications/${applicationId}/collector?language=${language}`, { waitUntil: 'domcontentloaded' });
      const install = page.getByRole('button', { name: /安装到 1flowbase|Install in 1flowbase/ });
      const download = page.getByRole('button', { name: /下载采集 CLI|Download collector CLI/ });
      await page.getByRole('tab', { name: 'Codex', exact: true }).waitFor({ state: 'visible', timeout: 30000 });
      let catalogResponse = await context.request.get(`${apiBaseUrl}/api/console/applications/catalog`);
      assert.equal(catalogResponse.status(), 200);
      let catalog = (await catalogResponse.json()).data;
      assert.deepEqual(catalog.collectors.map(item => item.collector_code), ['codex-logs-collector']);
      assert.equal(catalog.collectors[0].execution_target, 'client');
      await page.getByRole('tab', { name: 'Codex', exact: true }).click();
      if (catalog.collectors[0].installation_status === 'not_installed') {
        assert.equal(await page.locator('.application-collector__command pre').count(), 0);
        await page.screenshot({ animations: 'disabled', path: path.join(out, `${name}-uninstalled.png`), fullPage: true });
        await install.click();
        await page.getByText('macOS / Linux (Shell)', { exact: true }).waitFor({ timeout: 60000 });
        catalogResponse = await context.request.get(`${apiBaseUrl}/api/console/applications/catalog`);
        assert.equal(catalogResponse.status(), 200);
        catalog = (await catalogResponse.json()).data;
      }
      assert.equal(catalog.collectors[0].installation_status, 'installed');
      assert.equal(catalog.collectors[0].installed_version, '0.1.0');
      assert.ok(catalog.collectors[0].shell_installer_url.startsWith('/api/public/client-collectors/'));
      const collectorTab = page.getByRole('tab', { name: 'Codex', exact: true });
      const allTab = page.getByRole('tab', { name: /^(全部|All)$/ });
      await page.getByText('macOS / Linux (Shell)', { exact: true }).waitFor();
      assert.equal(await collectorTab.getAttribute('aria-selected'), 'true');
      assert.equal(await download.count(), 0, 'collector tab opens detail directly');
      await page.getByRole('button', { name: /返回|Back/ }).click();
      assert.equal(await allTab.getAttribute('aria-selected'), 'true');
      await download.waitFor({ state: 'visible' });
      await page.screenshot({ animations: 'disabled', path: path.join(out, `${name}-catalog.png`), fullPage: true });
      await download.click();
      assert.equal(await collectorTab.getAttribute('aria-selected'), 'true');
      await page.getByText('macOS / Linux (Shell)', { exact: true }).waitFor();
      let text = await page.locator('body').innerText();
      assert.ok(text.includes('bash "$installer"')); assert.ok(text.includes(applicationId));
      assert.ok(text.includes('/api/logs/v1/events')); assert.ok(!text.includes('?api_key='));
      assert.ok(!text.includes('github.com'));
      assert.ok(text.includes('--release-base'));
      const shellCommand = await page.locator('.application-collector__command pre').innerText();
      const endpoint = shellCommand.match(/--endpoint '([^']+)'/)?.[1];
      assert.match(endpoint || '', /^https?:\/\//);
      const releaseBase = shellCommand.match(/--release-base '([^']+)'/)?.[1];
      assert.equal(releaseBase, webBaseUrl + catalog.collectors[0].asset_base_url);
      assert.ok(shellCommand.includes(webBaseUrl + catalog.collectors[0].shell_installer_url));
      assert.ok(shellCommand.includes('--max-redirs 0'));
      assert.ok(new URL(endpoint).pathname.endsWith('/api/logs/v1/events'));
      if (process.env.COLLECTOR_EXPECTED_ENDPOINT) assert.equal(endpoint, process.env.COLLECTOR_EXPECTED_ENDPOINT);
      await page.getByRole('button', { name: /复制命令|Copy command/ }).click();
      await page.waitForFunction(expected => navigator.clipboard.readText().then(value => value === expected), shellCommand);
      const overflow = await page.evaluate(() => ({ width: document.documentElement.clientWidth, scroll: document.documentElement.scrollWidth }));
      assert.ok(overflow.scroll <= overflow.width + 1, `overflow ${JSON.stringify(overflow)}`);
      await page.waitForFunction(() => {
        const tab = document.querySelector('[role="tab"][aria-selected="true"]');
        const indicator = document.querySelector('.ant-tabs-ink-bar');
        if (!tab || !indicator) return false;
        const target = tab.getBoundingClientRect();
        const actual = indicator.getBoundingClientRect();
        return Math.abs(target.left + target.width / 2 - actual.left - actual.width / 2) < 1;
      });
      await page.screenshot({ animations: 'disabled', path: path.join(out, `${name}-shell.png`), fullPage: true });
      await page.getByText('Windows (PowerShell)', { exact: true }).click();
      text = await page.locator('body').innerText();
      assert.ok(text.includes('-Endpoint')); assert.ok(text.includes('-InstallationId'));
      assert.equal(text.match(/-Endpoint '([^']+)'/)?.[1], endpoint);
      assert.equal(text.match(/-ReleaseBase '([^']+)'/)?.[1], releaseBase);
      assert.ok(text.includes('-MaximumRedirection 0'));
      await page.screenshot({ animations: 'disabled', path: path.join(out, `${name}-powershell.png`), fullPage: true });
      const keys = page.getByRole('link', { name: /API Key/i }).first();
      assert.equal(await keys.getAttribute('href'), `/applications/${applicationId}/api`);
      await allTab.click();
      await download.waitFor();
      assert.equal(await page.locator('.application-collector__command pre').count(), 0);
      await collectorTab.click();
      await page.getByText('macOS / Linux (Shell)', { exact: true }).waitFor();
      assert.equal(await download.count(), 0);
      await page.getByRole('button', { name: /返回|Back/ }).click();
      await download.waitFor();
      assert.equal(await allTab.getAttribute('aria-selected'), 'true');
      assert.deepEqual(errors, []);
      receipts.push({ name, language, viewport, overflow, errors, status: 'pass', tab_navigation: 'collector tab and card open same detail; all and back restore directory', endpoint, clipboard: 'exact shell command', collector: catalog.collectors[0] });
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
