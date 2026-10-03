const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { exportApplicationTemplate, replaceDirectory, selectionOf } = require('../core.js');

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'template-export-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const target = path.join(root, '@test/demo');
  let pkg = { schema_version: '1flowbase.portable-template/v1', pages: [{ id: 'page-1', title: 'before' }], applications: [], data_models: [], plugins: [], mcp_bundle: { manifest: { exported_at: 'first' } } };
  let disposed = 0;
  let exportFailure = false;
  const requests = [];
  const archiveTools = {
    async splitPackage(value, directory) { fs.mkdirSync(directory, { recursive: true }); fs.writeFileSync(path.join(directory, 'manifest.json'), JSON.stringify({ package: value })); },
    async readPackage(directory) { return JSON.parse(fs.readFileSync(path.join(directory, 'manifest.json'))).package; },
  };
  const dependencies = { archiveTools, loadRootCredentials: () => ({ account: 'fixture', password: 'never-persist' }),
    openTemporaryOwnerSession: async () => ({ cookie: 'fixture-cookie', csrfToken: 'fixture-token', dispose: async () => { disposed++; } }),
    fetchImpl: async (url, options) => {
      if (url.endsWith('/health')) return { ok: true, json: async () => ({ version: '0.4.1' }) };
      requests.push(JSON.parse(options.body));
      if (exportFailure) return { ok: false, status: 503, text: async () => 'fixture unavailable' };
      pkg.mcp_bundle.manifest.exported_at = `export-${requests.length}`;
      return { ok: true, json: async () => ({ data: structuredClone(pkg) }) };
    } };
  const initial = { target, selection: { page_ids: ['page-1'], mcp_instance_ids: ['1flowbase'] }, name: 'Demo' };
  return { target, initial, dependencies, archiveTools, requests, disposed: () => disposed,
    change: () => { pkg.pages[0].title = 'after'; }, fail: () => { exportFailure = true; } };
}

test('first export saves canonical selection without credentials; update reuses it and increments only for changed content', async (t) => {
  const f = fixture(t);
  assert.equal((await exportApplicationTemplate(f.initial, f.dependencies)).release_version, 1);
  const savedBytes = fs.readFileSync(path.join(f.target, 'export.config.json'), 'utf8');
  assert.doesNotMatch(savedBytes, /never-persist|fixture-cookie|fixture-token/);
  assert.equal((await exportApplicationTemplate({ target: f.target }, f.dependencies)).changed, false);
  assert.equal((await f.archiveTools.readPackage(f.target)).release.release_version, 1);
  f.change();
  const updated = await exportApplicationTemplate({ target: f.target }, f.dependencies);
  assert.equal(updated.release_version, 2);
  assert.equal(updated.changed, true);
  assert.deepEqual(f.requests[0], f.requests[2]);
  assert.equal(f.disposed(), 3);
});

test('failed export preserves previous source and reclaims the session', async (t) => {
  const f = fixture(t);
  await exportApplicationTemplate(f.initial, f.dependencies);
  const before = fs.readFileSync(path.join(f.target, 'manifest.json'));
  f.fail();
  await assert.rejects(exportApplicationTemplate({ target: f.target }, f.dependencies), /503/);
  assert.deepEqual(fs.readFileSync(path.join(f.target, 'manifest.json')), before);
  assert.equal(f.disposed(), 2);
  assert.deepEqual(fs.readdirSync(path.dirname(f.target)), ['demo']);
});

test('atomic replacement restores old directory when the final rename fails', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'template-replace-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const target = path.join(root, 'target');
  const staged = path.join(root, 'staged');
  fs.mkdirSync(target); fs.mkdirSync(staged);
  fs.writeFileSync(path.join(target, 'sentinel'), 'old');
  assert.throws(() => replaceDirectory(staged, target, (from, to) => {
    if (from === staged) throw new Error('disk fixture');
    fs.renameSync(from, to);
  }), /disk fixture/);
  assert.equal(fs.readFileSync(path.join(target, 'sentinel'), 'utf8'), 'old');
});

test('rejects empty selections and noncanonical fields before network activity', () => {
  assert.throws(() => selectionOf({}), /at least one/);
  assert.throws(() => selectionOf({ app_ids: ['id'] }), /unknown/);
});
