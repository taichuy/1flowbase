'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { configuration } = require('../config.cjs');
const { isolatedObjectRoot, storageBinding, objectSnapshot, uploadFixture, multipart, uploadedRecord, readbackOracle, uploadAndRead } = require('../mixed-workload.cjs');
const storageId = '11111111-1111-1111-1111-111111111111';
const tableId = '22222222-2222-2222-2222-222222222222';
const recordId = '33333333-3333-3333-3333-333333333333';
const binding = { storageId, tableId };
const fixture = uploadFixture(1024 * 1024);
const record = () => ({ data: { file_table_id: tableId, storage_id: storageId, record: {
  id: recordId, size: fixture.size, mimetype: fixture.mime, filename: 'resource-benchmark.bin', storage_id: storageId,
} } });
const content = (bytes = fixture.bytes, headers = {}) => new Response(bytes, {
  headers: { 'content-type': fixture.mime, 'content-length': String(fixture.size), ...headers },
});

test('configuration defaults to text and only admits fixed mixed upload sizes', () => {
  const env = { RB_REPO_ROOT: '/fixture', RB_ARTIFACT_ROOT: '/fixture/tmp/test-governance/run', RB_DEDICATED_DATABASE: '1',
    RB_DATABASE_URL: 'postgres://localhost/qadb_test', RB_API_BINARY: '/binary', RB_OPENAI_PACKAGE: '/openai',
    RB_ANTHROPIC_PACKAGE: '/anthropic', RB_OPENAI_COMPATIBLE_PACKAGE: '/compatible' };
  assert.equal(configuration(env).workload, 'text');
  for (const size of [1048576, 8388608]) assert.equal(configuration({ ...env, RB_WORKLOAD: 'mixed', RB_UPLOAD_BYTES: String(size) }).uploadBytes, size);
  assert.throws(() => configuration({ ...env, RB_WORKLOAD: 'unknown' }));
  assert.throws(() => configuration({ ...env, RB_UPLOAD_BYTES: '2097152' }));
});

test('multipart sends exact binary file/MIME and omits table to exercise default selection', async () => {
  const form = multipart(fixture);
  assert.deepEqual([...form.keys()], ['file']);
  const file = form.get('file');
  assert.equal(file.name, 'resource-benchmark.bin');
  assert.equal(file.type, fixture.mime);
  assert.equal(file.size, fixture.size);
  assert.deepEqual(Buffer.from(await file.arrayBuffer()), fixture.bytes);
  assert.deepEqual(fixture.bytes.subarray(0, 256), Buffer.from(Array.from({ length: 256 }, (_, i) => i)));
});

test('upload metadata and readback oracle reject wrong binding, size, MIME and same-size corruption', async () => {
  assert.equal(uploadedRecord(record().data, fixture, binding), recordId);
  for (const change of [{ size: fixture.size - 1 }, { mimetype: 'text/plain' }, { storage_id: tableId }, { id: '../escape' }]) {
    const data = record().data;
    Object.assign(data.record, change);
    assert.throws(() => uploadedRecord(data, fixture, binding));
  }
  assert.throws(() => uploadedRecord({ ...record().data, file_table_id: storageId }, fixture, binding));
  await readbackOracle(content(), fixture);
  await assert.rejects(readbackOracle(content(fixture.bytes, { 'content-type': 'text/plain' }), fixture));
  await assert.rejects(readbackOracle(content(fixture.bytes, { 'content-length': '1' }), fixture));
  await assert.rejects(readbackOracle(content(fixture.bytes.subarray(1)), fixture));
  const corrupt = Buffer.from(fixture.bytes); corrupt[100] ^= 1;
  await assert.rejects(readbackOracle(content(corrupt), fixture));
});

test('upload/readback reuse temporary owner and return only safe measurements; upload failure is fatal', async () => {
  const calls = [];
  const client = { baseUrl: 'http://127.0.0.1:12345', cookie: 'private-cookie', csrf: 'private-csrf',
    fetch: async (url, options) => {
      calls.push({ url, options });
      return calls.length === 1 ? Response.json(record(), { status: 201 }) : content();
    } };
  const result = await uploadAndRead(client, fixture, binding);
  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, `${client.baseUrl}/api/console/files/upload`);
  assert.equal(calls[0].options.headers['x-csrf-token'], client.csrf);
  assert.equal(calls[1].url, `${client.baseUrl}/api/console/files/${tableId}/records/${recordId}/content`);
  assert.equal(calls[1].options.headers.cookie, client.cookie);
  assert.equal(result.sha256, fixture.sha256);
  assert.equal(result.upload_bytes, fixture.size);
  assert.ok(result.elapsed_ms >= result.upload_ms);
  assert.ok(!JSON.stringify(result).includes('private-'));
  await assert.rejects(uploadAndRead({ ...client, fetch: async () => new Response('private-body', { status: 413 }) }, fixture, binding), /mixed upload rejected \(413\)/);
});

test('owned cwd isolates default storage and rejects persisted foreign bindings and symlinks', () => {
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'resource-mixed-test-'));
  const repo = fs.mkdtempSync(path.join(os.tmpdir(), 'resource-mixed-repo-'));
  try {
    for (const name of ['host-extensions', 'capability-plugins', 'sets']) fs.mkdirSync(path.join(repo, 'api/plugins', name), { recursive: true });
    const root = isolatedObjectRoot(scratch, repo);
    assert.equal(fs.realpathSync(path.join(scratch, 'api/plugins/host-extensions')), path.join(repo, 'api/plugins/host-extensions'));
    assert.equal(root, path.join(fs.realpathSync(scratch), 'api', 'storage'));
    const storages = [{ id: storageId, is_default: true, enabled: true, driver_type: 'local', config_json: { root_path: root } }];
    const tables = [{ id: tableId, is_default: true, status: 'active', bound_storage_id: storageId }];
    assert.deepEqual(storageBinding(storages, tables, root), binding);
    assert.deepEqual(storageBinding(storages, [{ ...tables[0], bound_storage_id: null }], root), binding);
    assert.throws(() => storageBinding(storages, tables, path.join(repo, 'api/storage')));
    assert.throws(() => storageBinding(storages, [{ ...tables[0], bound_storage_id: tableId }], root));
    assert.throws(() => storageBinding([], tables, root));
    assert.throws(() => isolatedObjectRoot(repo, repo));
    fs.writeFileSync(path.join(root, 'file'), Buffer.alloc(37));
    fs.writeFileSync(path.join(root, 'file.metadata.json'), '{}');
    assert.equal(objectSnapshot(root).bytes, 39);
    assert.equal(objectSnapshot(root).files, 2);
    fs.symlinkSync(repo, path.join(root, 'outside'));
    assert.throws(() => objectSnapshot(root), /symlink/);
  } finally {
    fs.rmSync(scratch, { recursive: true, force: true });
    fs.rmSync(repo, { recursive: true, force: true });
  }
});
