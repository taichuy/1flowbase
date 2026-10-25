'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const UPLOAD_SIZES = [1024 * 1024, 8 * 1024 * 1024];
const MIME = 'application/octet-stream';
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function isolatedObjectRoot(scratchRoot, repoRoot) {
  const scratch = fs.realpathSync(scratchRoot);
  const repo = fs.realpathSync(repoRoot);
  if (scratch === repo || scratch.startsWith(`${repo}${path.sep}`)) throw Error('mixed cwd must be outside source tree');
  // Match config.rs find_workspace_root: no ancestor may redirect default storage.
  for (let dir = scratch; ; dir = path.dirname(dir)) {
    if (fs.existsSync(path.join(dir, '.git')) && fs.existsSync(path.join(dir, 'api')) && fs.existsSync(path.join(dir, 'web'))) throw Error('mixed cwd resolves to a workspace');
    if (path.dirname(dir) === dir) break;
  }
  const objects = path.join(scratch, 'api', 'storage');
  fs.mkdirSync(objects, { recursive: true });
  if (fs.realpathSync(objects) !== objects) throw Error('mixed object directory must not follow symlinks');
  // api_workspace_root also needs static builtins when the binary build tree is absent.
  // Link only source assets; provider installations and all writable state stay private.
  const plugins = path.join(scratch, 'api', 'plugins');
  fs.mkdirSync(plugins, { recursive: true });
  for (const name of ['host-extensions', 'capability-plugins', 'sets']) {
    const source = path.join(repo, 'api', 'plugins', name);
    if (!fs.statSync(source).isDirectory()) throw Error('mixed builtin source assets unavailable');
    fs.symlinkSync(source, path.join(plugins, name), 'dir');
  }
  return objects;
}

// Reject a reused DB whose persisted storage binding points outside owned scratch.
function storageBinding(storages, tables, objectRoot) {
  if (!Array.isArray(storages) || !Array.isArray(tables)) throw Error('mixed storage discovery shape mismatch');
  const defaults = storages.filter(s => s.is_default);
  const defaultTables = tables.filter(t => t.is_default && t.status === 'active');
  if (defaults.length !== 1 || defaultTables.length !== 1) throw Error('mixed default storage/table unavailable or ambiguous');
  const storage = defaults[0], table = defaultTables[0];
  if (!UUID.test(storage.id) || !UUID.test(table.id) || storage.driver_type !== 'local' || storage.enabled !== true ||
      storage.config_json?.root_path !== objectRoot ||
      (table.bound_storage_id !== null && table.bound_storage_id !== storage.id)) throw Error('mixed storage is not isolated');
  return { storageId: storage.id, tableId: table.id };
}
async function verifyStorage(client, objectRoot) {
  const [{ data: storages }, { data: tables }] = await Promise.all([
    client.read('/api/console/settings/files/storages'), client.read('/api/console/settings/files/tables'),
  ]);
  return storageBinding(storages, tables, objectRoot);
}
function objectSnapshot(root) {
  let bytes = 0, allocated = 0, files = 0;
  function visit(dir) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      const stat = fs.lstatSync(full);
      if (stat.isSymbolicLink()) throw Error('mixed object snapshot encountered symlink');
      if (stat.isDirectory()) visit(full);
      else if (stat.isFile()) { files++; bytes += stat.size; allocated += stat.blocks * 512; }
      else throw Error('mixed object snapshot encountered non-file');
    }
  }
  visit(root);
  return { files, bytes, allocated_bytes: allocated, includes_metadata_sidecars: true };
}
function uploadFixture(size) {
  if (!UPLOAD_SIZES.includes(size)) throw Error('unsupported RB_UPLOAD_BYTES');
  const bytes = Buffer.alloc(size);
  for (let i = 0; i < size; i++) bytes[i] = i % 256;
  return { bytes, size, mime: MIME, sha256: crypto.createHash('sha256').update(bytes).digest('hex') };
}
function multipart(fixture) {
  const form = new FormData();
  form.append('file', new Blob([fixture.bytes], { type: fixture.mime }), 'resource-benchmark.bin');
  return form;
}
function uploadedRecord(data, fixture, binding) {
  const record = data?.record;
  if (data?.file_table_id !== binding.tableId || data?.storage_id !== binding.storageId ||
      !UUID.test(record?.id) || record?.size !== fixture.size || record?.mimetype !== fixture.mime ||
      record?.filename !== 'resource-benchmark.bin' || record?.storage_id !== binding.storageId) throw Error('mixed upload metadata mismatch');
  return record.id;
}
async function readbackOracle(response, fixture) {
  if (response.status !== 200 || response.headers.get('content-type') !== fixture.mime ||
      response.headers.get('content-length') !== String(fixture.size)) {
    await response.body?.cancel();
    throw Error('mixed readback status/size/MIME mismatch');
  }
  const hash = crypto.createHash('sha256');
  let size = 0;
  for await (const chunk of response.body) {
    size += chunk.length;
    if (size > fixture.size) throw Error('mixed readback oversized');
    hash.update(chunk);
  }
  if (size !== fixture.size || hash.digest('hex') !== fixture.sha256) throw Error('mixed readback SHA256/size mismatch');
}
async function uploadAndRead(client, fixture, binding) {
  if (!client?.cookie || !client?.csrf) throw Error('mixed temporary owner session unavailable');
  const started = performance.now();
  const response = await client.fetch(`${client.baseUrl}/api/console/files/upload`, {
    method: 'POST', headers: { cookie: client.cookie, 'x-csrf-token': client.csrf },
    body: multipart(fixture), signal: AbortSignal.timeout(30000), redirect: 'error',
  });
  if (response.status !== 201) { await response.body?.cancel(); throw Error(`mixed upload rejected (${response.status})`); }
  let payload;
  try { payload = await response.json(); } catch { throw Error('mixed upload returned invalid JSON'); }
  const recordId = uploadedRecord(payload?.data, fixture, binding);
  const uploadMs = performance.now() - started;
  const readStart = performance.now();
  const read = await client.fetch(`${client.baseUrl}/api/console/files/${binding.tableId}/records/${recordId}/content`, {
    headers: { cookie: client.cookie }, signal: AbortSignal.timeout(30000), redirect: 'error',
  });
  await readbackOracle(read, fixture);
  return { upload_ms: uploadMs, readback_ms: performance.now() - readStart, elapsed_ms: performance.now() - started,
    upload_bytes: fixture.size, readback_bytes: fixture.size, sha256: fixture.sha256, mime: fixture.mime };
}
module.exports = { UPLOAD_SIZES, isolatedObjectRoot, storageBinding, verifyStorage, objectSnapshot, uploadFixture, multipart, uploadedRecord, readbackOracle, uploadAndRead };
