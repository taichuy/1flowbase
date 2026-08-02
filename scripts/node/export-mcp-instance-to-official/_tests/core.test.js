const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const {
  exportMcpInstanceToOfficial,
  extractBundleArchive,
  nextPatch,
  replaceDirectoryAtomically,
} = require('../core.js');
const { readArguments } = require('../cli.js');

function zip(entries) {
  const local = [];
  const central = [];
  let localOffset = 0;
  for (const entry of entries) {
    const name = Buffer.from(entry.name);
    const contents = Buffer.from(entry.contents || '');
    const header = Buffer.alloc(30);
    header.writeUInt32LE(0x04034b50, 0);
    header.writeUInt16LE(20, 4);
    header.writeUInt32LE(contents.length, 18);
    header.writeUInt32LE(contents.length, 22);
    header.writeUInt16LE(name.length, 26);
    local.push(header, name, contents);
    const record = Buffer.alloc(46);
    record.writeUInt32LE(0x02014b50, 0);
    record.writeUInt16LE(0x031e, 4);
    record.writeUInt16LE(20, 6);
    record.writeUInt32LE(contents.length, 20);
    record.writeUInt32LE(contents.length, 24);
    record.writeUInt16LE(name.length, 28);
    record.writeUInt32LE(((entry.mode ?? 0o100644) << 16) >>> 0, 38);
    record.writeUInt32LE(localOffset, 42);
    central.push(record, name);
    localOffset += header.length + name.length + contents.length;
  }
  const centralBytes = Buffer.concat(central);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(centralBytes.length, 12);
  end.writeUInt32LE(localOffset, 16);
  return Buffer.concat([...local, centralBytes, end]);
}

function fixtureTarget() {
  const parent = fs.mkdtempSync(path.join(os.tmpdir(), 'mcp-export-cli-'));
  const target = path.join(parent, 'mcp', '@taichuy', 'example');
  fs.mkdirSync(path.join(target, 'tools'), { recursive: true });
  fs.writeFileSync(path.join(target, 'manifest.json'), JSON.stringify({
    schema_version: '1flowbase.mcp.bundle/v2',
    organization: 'taichuy',
    bundle_id: 'example',
    bundle_version: '1.2.3',
    locale: 'zh_Hans',
    minimum_host_version: '0.3.0',
    exported_from_system_version: '0.3.0',
  }));
  fs.writeFileSync(path.join(target, 'tools', 'stale.json'), '{}');
  return { parent, target };
}

function exportedArchive() {
  return zip([
    { name: 'manifest.json', contents: JSON.stringify({
      schema_version: '1flowbase.mcp.bundle/v2',
      organization: 'taichuy',
      bundle_id: 'example',
      bundle_version: '1.2.4',
      locale: 'zh_Hans',
      minimum_host_version: '0.3.1',
      exported_from_system_version: '0.3.1',
      files: [],
    }) },
    { name: 'tools/new.json', contents: '{}' },
    { name: 'instances/instance.json', contents: '{}' },
  ]);
}

test('AC-001 bumps only the patch component', () => {
  assert.equal(nextPatch('1.2.3'), '1.2.4');
  assert.throws(() => nextPatch('1.2'), /semantic version/);
});

test('CLI help short-circuits and the API defaults to the local api-server port', () => {
  assert.deepEqual(readArguments(['--help']), { help: true });
  assert.deepEqual(readArguments(['-h']), { help: true });
  assert.equal(readArguments([
    '--instance-id', 'example', '--target', '/tmp/mcp/@taichuy/example',
  ]).apiBaseUrl, 'http://127.0.0.1:7800');
});

test('AC-002 rejects path traversal and symlink ZIP entries', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'mcp-safe-zip-'));
  assert.throws(() => extractBundleArchive(zip([{ name: '../outside', contents: 'x' }]), root), /unsafe ZIP entry path/);
  assert.throws(
    () => extractBundleArchive(zip([{ name: 'tools/link', contents: '../target', mode: 0o120777 }]), root),
    /symlink entries are not allowed/,
  );
});

test('AC-002 exports with a temporary owner session, validates, atomically replaces, and disposes', async () => {
  const { parent, target } = fixtureTarget();
  let disposed = 0;
  let requestBody;
  let credentialsRepoRoot;
  const archive = exportedArchive();
  const result = await exportMcpInstanceToOfficial({
    instanceId: 'system/example', target, apiBaseUrl: 'http://127.0.0.1:3000/',
  }, {
    loadRootCredentials: ({ repoRoot }) => {
      credentialsRepoRoot = repoRoot;
      return { account: 'root', password: 'secret' };
    },
    openTemporaryOwnerSession: async () => ({
      cookie: 'session=test', csrfToken: 'csrf', async dispose() { disposed += 1; },
    }),
    fetchImpl: async (url, request) => {
      assert.match(url, /system%2Fexample\/bundles\/export$/);
      requestBody = JSON.parse(request.body);
      return { ok: true, status: 200, arrayBuffer: async () => archive };
    },
    validateSource: async (bundleRoot) => {
      assert.equal(path.basename(path.dirname(bundleRoot)), '@taichuy');
      assert.equal(path.basename(bundleRoot), 'example');
      assert.equal(JSON.parse(fs.readFileSync(path.join(bundleRoot, 'manifest.json'))).bundle_version, '1.2.4');
    },
  });
  assert.deepEqual(requestBody, {
    organization: 'taichuy', bundle_id: 'example', bundle_version: '1.2.4', locale: 'zh_Hans',
  });
  assert.equal(result.exported_from_system_version, '0.3.1');
  assert.equal(credentialsRepoRoot, path.resolve(__dirname, '../../../..'));
  assert.equal(disposed, 1);
  assert.equal(fs.existsSync(path.join(target, 'tools', 'stale.json')), false);
  assert.equal(fs.existsSync(path.join(target, 'tools', 'new.json')), true);
  fs.rmSync(parent, { recursive: true, force: true });
});

test('AC-002 disposes the owner session and preserves the target when validation fails', async () => {
  const { parent, target } = fixtureTarget();
  const before = fs.readFileSync(path.join(target, 'manifest.json'), 'utf8');
  let disposed = 0;
  await assert.rejects(() => exportMcpInstanceToOfficial({
    instanceId: 'example', target, apiBaseUrl: 'http://127.0.0.1:3000',
  }, {
    loadRootCredentials: () => ({ account: 'root', password: 'secret' }),
    openTemporaryOwnerSession: async () => ({
      cookie: 'session=test', csrfToken: 'csrf', async dispose() { disposed += 1; },
    }),
    fetchImpl: async () => ({ ok: true, status: 200, arrayBuffer: async () => exportedArchive() }),
    validateSource: async () => { throw new Error('validator rejected source'); },
  }), /validator rejected source/);
  assert.equal(disposed, 1);
  assert.equal(fs.readFileSync(path.join(target, 'manifest.json'), 'utf8'), before);
  fs.rmSync(parent, { recursive: true, force: true });
});

test('AC-002 restores the old target when the second atomic rename fails', () => {
  const { parent, target } = fixtureTarget();
  const staged = path.join(path.dirname(target), 'staged');
  fs.mkdirSync(staged);
  fs.writeFileSync(path.join(staged, 'manifest.json'), '{"new":true}');
  let calls = 0;
  assert.throws(() => replaceDirectoryAtomically(staged, target, {
    renameSync(from, to) {
      calls += 1;
      if (calls === 2) throw new Error('simulated rename failure');
      fs.renameSync(from, to);
    },
  }), /simulated rename failure/);
  assert.equal(JSON.parse(fs.readFileSync(path.join(target, 'manifest.json'))).bundle_version, '1.2.3');
  fs.rmSync(parent, { recursive: true, force: true });
});
