#!/usr/bin/env node
// Candidate-bound source integration, exclusively in an owned temporary PostgreSQL schema.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const net = require('node:net');
const assert = require('node:assert/strict');
const { spawn, spawnSync } = require('node:child_process');
const { createRequire } = require('node:module');
const { openTemporaryOwnerSession } = require('../page-debug/auth.js');
const root = path.resolve(__dirname, '../../..');
const output = path.join(root, 'tmp/test-governance/2014/browser');
const schema = `root2014_${crypto.randomBytes(10).toString('hex')}`;
const report = { candidate: null, schema, status: 'not_run', scenarios: [], screenshots: [], errors: [], processes: [] };
const children = new Set();
const sessions = new Set();
let schemaCreated = false;
let browser;
let pgEnv;
fs.mkdirSync(output, { recursive: true });
const runtime = fs.mkdtempSync(path.join(output, 'runtime-'));
// Optimized chunks must resolve external dependencies through the app's node_modules ancestry.
const viteCache = fs.mkdtempSync(path.join(root, 'web/app/node_modules/.vite-root2014-'));
report.viteCache = viteCache;
const digest = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
function command(program, args, options = {}) {
  const result = spawnSync(program, args, { cwd: root, encoding: 'utf8', maxBuffer: 4 * 1024 * 1024, ...options });
  if (result.status !== 0) throw new Error(`${program} failed: ${(result.stderr || '').slice(-1500)}`);
  return result.stdout.trim();
}
function sql(statement) {
  return command(process.env.ROOT_2014_PSQL || 'psql', ['-X', '-v', 'ON_ERROR_STOP=1', '-At', '-c', statement], { env: pgEnv });
}
async function freePort() {
  const server = net.createServer();
  await new Promise((resolve, reject) => server.listen(0, '127.0.0.1', resolve).once('error', reject));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  return port;
}
function launch(label, program, args, env, cwd = root) {
  const filename = path.join(output, `${label}-${Date.now()}.log`);
  const fd = fs.openSync(filename, 'w');
  const child = spawn(program, args, { cwd, env, stdio: ['ignore', fd, fd], detached: true });
  fs.closeSync(fd);
  child.on('error', error => { child.launchError = error; });
  children.add(child);
  report.processes.push({ label, pid: child.pid, log: path.relative(root, filename) });
  return child;
}
async function stop(child) {
  if (!child || !children.has(child)) return;
  if (child.exitCode === null && !child.launchError) {
    try { process.kill(-child.pid, 'SIGTERM'); } catch (error) { if (error.code !== 'ESRCH') throw error; }
    for (let attempt = 0; attempt < 100 && child.exitCode === null && child.signalCode === null; attempt++) await delay(100);
    if (child.exitCode === null && child.signalCode === null) {
      try { process.kill(-child.pid, 'SIGKILL'); } catch (error) { if (error.code !== 'ESRCH') throw error; }
      await new Promise(resolve => child.once('exit', resolve));
    }
  }
  children.delete(child);
}
async function ready(base, child, web = false) {
  for (let attempt = 0; attempt < 180; attempt++) {
    if (child.launchError || child.exitCode !== null) throw new Error('owned candidate service exited before readiness; see log');
    try {
      const response = await fetch(`${base}${web ? '/__1flowbase_dev_ready' : '/health'}`, { signal: AbortSignal.timeout(1500) });
      if (web ? response.ok && (await response.json()).state === 'Ready' : response.status < 500) return;
    } catch {}
    await delay(500);
  }
  throw new Error('owned candidate service readiness timeout');
}
async function disposeSession(session) {
  if (!session || !sessions.has(session)) return;
  try { await session.dispose(); } finally { sessions.delete(session); }
}
async function session(base, password) {
  const value = await openTemporaryOwnerSession({ apiBaseUrl: base, account: 'root2014', password });
  sessions.add(value);
  return value;
}
async function request(base, owner, route, { method = 'GET', body, form, expected = 200 } = {}) {
  const headers = { cookie: owner.cookie, 'x-csrf-token': owner.csrfToken };
  if (body !== undefined) headers['content-type'] = 'application/json';
  const response = await fetch(`${base}${route}`, { method, headers, body: form || (body === undefined ? undefined : JSON.stringify(body)) });
  const text = await response.text();
  if (expected === null) return { status: response.status, text };
  assert.ok([].concat(expected).includes(response.status), `${method} ${route}: ${response.status}; ${text.slice(0, 900)}`);
  return text ? JSON.parse(text).data : null;
}
const pageRoute = '/api/console/settings/ui-management/plugin-settings-page?route_id=northwind.settings-page.settings';
const templateRoute = '/api/console/settings/ui-management/templates';
function archive(version, source) {
  const directory = path.join(runtime, `package-${version}`);
  fs.cpSync(path.join(root, 'api/plugins/fixtures/northwind.settings-page'), directory, { recursive: true });
  for (const filename of ['manifest.yaml', 'host-extension.yaml']) {
    const file = path.join(directory, filename);
    fs.writeFileSync(file, fs.readFileSync(file, 'utf8').replaceAll('1.0.0', version));
  }
  fs.writeFileSync(path.join(directory, 'settings.tsx'), source);
  const filename = path.join(runtime, `northwind-${version}.1flowbasepkg`);
  command('tar', ['-czf', filename, '-C', directory, '.']);
  return filename;
}
async function install(base, owner, filename) {
  const form = new FormData();
  form.append('file', new Blob([fs.readFileSync(filename)], { type: 'application/gzip' }), path.basename(filename));
  const result = await request(base, owner, '/api/console/plugins/install-upload', { method: 'POST', form, expected: [200, 201] });
  assert.match(result.installation?.id || '', /^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/u, 'formal install returns exact UUID');
  return result.installation.id;
}
async function enable(base, owner, id, action = 'enable') {
  await request(base, owner, `/api/console/settings/extension-center/installed/${id}/${action}`, { method: 'POST', expected: [200, 201, 202] });
}
async function cookieContext(owner, web, viewport) {
  const context = await browser.newContext({ viewport });
  await context.addCookies(owner.cookie.split(';').map(item => {
    const separator = item.indexOf('=');
    return { name: item.slice(0, separator).trim(), value: item.slice(separator + 1), url: web };
  }));
  return context;
}
async function screenshot(page, name) {
  const file = path.join(output, `${name}.png`);
  await page.screenshot({ path: file, fullPage: true });
  report.screenshots.push(path.relative(root, file));
}
async function viewPage(owner, web, sentinel, phase) {
  for (const [name, viewport] of [['desktop', { width: 1440, height: 1000 }], ['mobile', { width: 390, height: 844 }]]) {
    const context = await cookieContext(owner, web, viewport);
    try {
      const page = await context.newPage();
      await page.goto(`${web}/settings/northwind.settings-page`);
      await page.getByTestId('plugin-settings-page').getByText(sentinel, { exact: true }).waitFor({ timeout: 60000 });
      await screenshot(page, `${phase}-${name}`);
    } finally { await context.close(); }
  }
}
async function editTemplate(base, owner, web, source) {
  const current = await request(base, owner, pageRoute);
  const context = await cookieContext(owner, web, { width: 1440, height: 1000 });
  try {
    const page = await context.newPage();
    await page.goto(`${web}/settings/ui-management/code-templates`);
    const row = page.locator(`[data-template-id="${current.template_id}"]`);
    await row.getByRole('button', { name: /^(Edit|编辑)$/u }).click();
    const studio = page.getByTestId('ui-code-template-studio');
    await studio.getByTestId('plugin-settings-overwrite-warning').waitFor();
    await screenshot(page, `edit-warning-${Date.now()}`);
    const editor = studio.locator('.monaco-editor').first();
    await editor.click();
    await page.keyboard.press('ControlOrMeta+A');
    await page.keyboard.insertText(source);
    await studio.getByRole('button', { name: /^(Save|保存)$/iu }).click();
    await studio.waitFor({ state: 'hidden' });
    await row.getByRole('button', { name: /^(Publish|发布)$/u }).click();
    for (let attempt = 0; attempt < 40; attempt++) {
      const actual = await request(base, owner, pageRoute);
      if (actual.source === source) return actual;
      await delay(250);
    }
    throw new Error('UI edit/publish did not change the formal published source');
  } finally { await context.close(); }
}
async function chooseUpgrade(owner, web, installationId) {
  const context = await cookieContext(owner, web, { width: 1440, height: 1000 });
  try {
    const page = await context.newPage();
    await page.goto(`${web}/settings/extension-center/installed`);
    await page.getByTestId('plugin-settings-upgrade-warning').waitFor();
    await screenshot(page, 'upgrade-warning');
    const row = page.getByRole('row').filter({ hasText: 'northwind.settings-page' }).first();
    await row.getByRole('button', { name: /^(View|查看)$/u }).click();
    await page.locator(`li[data-installation-id="${installationId}"]`).getByRole('button', { name: /^(Select this version|选择此版本)$/u }).click();
    await page.locator('[data-testid="plugin-availability-status"][data-availability-status="pending_restart"]').first().waitFor();
    await screenshot(page, 'selected-version-pending-restart');
  } finally { await context.close(); }
}
async function main() {
  const candidate = process.argv[process.argv.indexOf('--candidate') + 1];
  assert.match(candidate || '', /^[a-f0-9]{40}$/u);
  assert.equal(command('git', ['rev-parse', 'HEAD']), candidate);
  assert.equal(command('git', ['status', '--porcelain', '--untracked-files=no']), '');
  report.candidate = candidate;
  const binary = path.resolve(process.env.ROOT_2014_API_BINARY || 'missing-candidate-binary');
  const identity = JSON.parse(fs.readFileSync(`${binary}.identity.json`, 'utf8'));
  assert.equal(identity.candidate, candidate, 'CI API binary must belong to the assembled SHA');
  assert.equal(identity.sha256, digest(binary));
  report.binary = identity;
  const database = new URL(process.env.ROOT_2014_DATABASE_URL || 'missing://database');
  assert.ok(['localhost', '127.0.0.1', '[::1]'].includes(database.hostname), 'browser fixture only uses local development PostgreSQL');
  pgEnv = { ...process.env, PGHOST: database.hostname, PGPORT: database.port || '5432', PGUSER: decodeURIComponent(database.username), PGPASSWORD: decodeURIComponent(database.password), PGDATABASE: database.pathname.slice(1) };
  delete pgEnv.PGOPTIONS;
  sql(`create schema "${schema}"`);
  schemaCreated = true;
  database.searchParams.set('options', `-csearch_path=${schema}`);
  const password = crypto.randomBytes(24).toString('hex');
  const ports = { a: await freePort(), b: await freePort(), web: await freePort() };
  const bases = Object.fromEntries(Object.entries(ports).map(([key, port]) => [key, `http://127.0.0.1:${port}`]));
  const env = { ...process.env, API_ENV: 'development', API_DATABASE_URL: database.toString(), DATABASE_URL: database.toString(), BOOTSTRAP_WORKSPACE_NAME: 'Root 2014 isolated acceptance', BOOTSTRAP_ROOT_ACCOUNT: 'root2014', BOOTSTRAP_ROOT_PASSWORD: password, BOOTSTRAP_ROOT_EMAIL: 'root2014@example.invalid', API_COOKIE_SECURE: 'false', API_PROVIDER_INSTALL_ROOT: path.join(runtime, 'installed'), API_HOST_EXTENSION_DROPIN_ROOT: path.join(runtime, 'dropins'), API_PLUGIN_ALLOW_UPLOADED_HOST_EXTENSIONS: 'true', API_PLUGIN_ALLOW_UNVERIFIED_FILESYSTEM_DROPINS: 'false', API_ALLOWED_ORIGINS: bases.web, API_PROVIDER_SECRET_MASTER_KEY: crypto.randomBytes(32).toString('hex') };
  let a; let b; let ownerA; let ownerB;
  const startApi = async node => {
    const child = launch(`api-${node}`, binary, [], { ...env, API_SERVER_ADDR: `127.0.0.1:${ports[node]}`, API_NODE_ID: `${schema}-${node}` });
    await ready(bases[node], child);
    return child;
  };
  const restartA = async () => { await disposeSession(ownerA); await stop(a); a = await startApi('a'); ownerA = await session(bases.a, password); };
  a = await startApi('a'); ownerA = await session(bases.a, password);
  const web = launch('web', 'pnpm', ['--dir', path.join(root, 'web/app'), 'exec', 'vite', '--host', '127.0.0.1', '--port', String(ports.web), '--strictPort'], { ...process.env, VITE_API_BASE_URL: '', VITE_API_PROXY_TARGET: bases.a, VITE_DEV_SERVER_PORT: String(ports.web), VITE_DEV_CACHE_DIR: viteCache }, path.join(root, 'web/app'));
  await ready(bases.web, web, true);
  const playwright = createRequire(path.join(root, 'web/package.json'))('playwright');
  browser = await playwright.chromium.launch({ headless: true, ...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH } : {}) });
  const source = sentinel => `export default function SettingsPage() { return <section><h1>Northwind settings</h1><p>${sentinel}</p></section>; }\n`;
  const v1 = await install(bases.a, ownerA, archive('1.0.0', source('Root 2014 version one')));
  await enable(bases.a, ownerA, v1);
  await restartA();
  await viewPage(ownerA, bases.web, 'Root 2014 version one', 'first-enable');
  await editTemplate(bases.a, ownerA, bases.web, source('Root 2014 user edit before upgrade'));
  await restartA();
  assert.equal((await request(bases.a, ownerA, pageRoute)).source, source('Root 2014 user edit before upgrade'));
  await enable(bases.a, ownerA, v1, 'disable');
  await enable(bases.a, ownerA, v1);
  await restartA();
  assert.equal((await request(bases.a, ownerA, pageRoute)).source, source('Root 2014 user edit before upgrade'));
  // A separately owned node retains its immutable v1 process while v2 is applied elsewhere.
  b = await startApi('b'); ownerB = await session(bases.b, password);
  await request(bases.b, ownerB, `/api/console/plugins/${v1}/artifact/install-current-node`, { method: 'POST', expected: [200, 201] });
  await disposeSession(ownerB); await stop(b); b = await startApi('b'); ownerB = await session(bases.b, password);
  assert.equal((await request(bases.b, ownerB, pageRoute)).applied_plugin_version, '1.0.0');
  const v2 = await install(bases.a, ownerA, archive('2.0.0', source('Root 2014 version two')));
  await chooseUpgrade(ownerA, bases.web, v2);
  const pending = sql(`select desired_state from "${schema}".extension_installations where id='${v2}'::uuid`);
  assert.equal(pending, 'pending_restart');
  const oldStatus = sql(`select runtime_status from "${schema}".extension_artifact_instances where installation_id='${v1}'::uuid and node_id='${schema}-a'`);
  assert.equal(oldStatus, 'active');
  await restartA();
  await viewPage(ownerA, bases.web, 'Root 2014 version two', 'upgrade');
  const stale = await request(bases.b, ownerB, pageRoute, { expected: null });
  assert.equal(stale.status, 409, 'old process must not serve v2 template');
  report.scenarios.push({ id: 'root_2014_ac_014_old_process_version_gate', status: 'passed', pending, oldRuntimeStatus: oldStatus, staleSourceStatus: stale.status });
  await editTemplate(bases.a, ownerA, bases.web, source('Root 2014 user edit after upgrade'));
  await restartA();
  await viewPage(ownerA, bases.web, 'Root 2014 user edit after upgrade', 'ordinary-restart');
  assert.equal((await request(bases.a, ownerA, pageRoute)).source, source('Root 2014 user edit after upgrade'));
  report.scenarios.push({ id: 'root_2014_ac_012_edit_upgrade_restart', status: 'passed', firstEnable: v1, upgrade: v2, viewports: ['desktop', 'mobile'], sameVersionReenablePreserved: true });
  report.status = 'passed';
}
main().catch(error => { report.status = 'failed'; report.errors.push(error.stack || String(error)); }).finally(async () => {
  if (browser) try { await browser.close(); } catch (error) { report.errors.push(`browser cleanup: ${error.message}`); }
  for (const owner of sessions) try { await disposeSession(owner); } catch (error) { report.errors.push(`session cleanup: ${error.message}`); }
  for (const child of children) try { await stop(child); } catch (error) { report.errors.push(`process cleanup: ${error.message}`); }
  try { fs.rmSync(viteCache, { recursive: true, force: true }); } catch (error) { report.errors.push(`cache cleanup: ${error.message}`); }
  if (schemaCreated) try { sql(`drop schema "${schema}" cascade`); } catch (error) { report.errors.push(`schema cleanup: ${error.message}`); }
  if (report.errors.length) report.status = 'failed';
  report.finishedAt = new Date().toISOString();
  fs.writeFileSync(path.join(output, 'report.json'), `${JSON.stringify(report, null, 2)}\n`);
  process.stdout.write(`Root 2014 browser: ${report.status}; ${path.relative(root, output)}/report.json\n`);
  process.exitCode = report.status === 'passed' ? 0 : 1;
});
