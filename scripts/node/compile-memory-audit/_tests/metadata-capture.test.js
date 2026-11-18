const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { test } = require('node:test');
const { JAVASCRIPT, commandArgs, parseArgs, readCapturedMetadata } = require('../metadata-capture.js');

const apiDir = path.resolve(__dirname, '../../../../api');
const directory = process.env.ONEFLOWBASE_DEPENDENCY_METADATA_TEST_DIR;
const integration = { skip: !directory && 'Set ONEFLOWBASE_DEPENDENCY_METADATA_TEST_DIR to a real --capture directory' };

function fixture(t) {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'dependency-metadata-'));
  t.after(() => fs.rmSync(temporary, { recursive: true, force: true }));
  const copy = path.join(temporary, 'capture');
  fs.cpSync(directory, copy, { recursive: true });
  return copy;
}

function changeReceipt(copy, change, feature = 'default') {
  const file = path.join(copy, `${feature}.receipt.json`);
  const receipt = JSON.parse(fs.readFileSync(file));
  change(receipt);
  fs.writeFileSync(file, JSON.stringify(receipt));
}

function load(copy, feature = 'default') {
  return readCapturedMetadata({ apiDir, directory: copy, feature });
}

test('CLI requests exact locked offline graphs and requires an explicit directory', () => {
  assert.deepEqual(commandArgs('/workspace/api', JAVASCRIPT), ['metadata', '--features', JAVASCRIPT,
    '--locked', '--offline', '--format-version', '1', '--manifest-path', '/workspace/api/Cargo.toml']);
  assert.equal(parseArgs(['--read', '--directory', '/captures']).feature, 'default');
  for (const args of [[], ['--read'], ['--read', '--directory', '/a', '--feature', 'wrong'],
    ['--capture', '--directory', '/a', '--feature', 'default'],
    ['--read', '--directory', '/a', '--directory', '/b']]) {
    assert.throws(() => parseArgs(args));
  }
});

test('missing capture directory fails closed', () => {
  assert.throws(() => load(path.join(os.tmpdir(), 'absent-dependency-metadata-capture')), /ENOENT/);
});

test('both genuine Cargo graphs round-trip byte for byte', integration, () => {
  for (const [feature, name] of [['default', 'default'], [JAVASCRIPT, 'javascript']]) {
    assert.deepEqual(load(directory, feature), fs.readFileSync(path.join(directory, `${name}.metadata.json`)));
  }
});

test('stale root manifest bytes are rejected', integration, (t) => {
  const copy = fixture(t);
  changeReceipt(copy, (receipt) => {
    const input = receipt.inputs.find((input) => input.path === path.join(apiDir, 'Cargo.toml'));
    input.contentBase64 = Buffer.concat([Buffer.from(input.contentBase64, 'base64'), Buffer.from('\n')]).toString('base64');
  });
  assert.throws(() => load(copy), /Stale captured metadata input/);
});

test('stale lock bytes are rejected', integration, (t) => {
  const copy = fixture(t);
  changeReceipt(copy, (receipt) => {
    receipt.inputs.find((input) => input.path.endsWith('/Cargo.lock')).contentBase64 = Buffer.from('stale lock').toString('base64');
  });
  assert.throws(() => load(copy), /Stale captured metadata input/);
});

test('stale local path package manifest bytes are rejected', integration, (t) => {
  const copy = fixture(t);
  changeReceipt(copy, (receipt) => {
    receipt.inputs.find((input) => input.path !== path.join(apiDir, 'Cargo.toml') && input.path.endsWith('/Cargo.toml'))
      .contentBase64 = Buffer.from('stale local package').toString('base64');
  });
  assert.throws(() => load(copy), /Stale captured metadata input/);
});

test('missing JavaScript graph never falls back to default or Cargo', integration, (t) => {
  const copy = fixture(t);
  fs.unlinkSync(path.join(copy, 'javascript.receipt.json'));
  assert.throws(() => load(copy, JAVASCRIPT), /ENOENT/);
  assert.doesNotThrow(() => load(copy));
});

test('wrong feature receipt is rejected', integration, (t) => {
  const copy = fixture(t);
  fs.copyFileSync(path.join(copy, 'default.receipt.json'), path.join(copy, 'javascript.receipt.json'));
  assert.throws(() => load(copy, JAVASCRIPT), /provenance mismatch/);
});

test('source SHA, workspace root and generator identity must match', integration, (t) => {
  for (const key of ['sourceSha', 'workspaceRoot', 'repoRoot', 'generatorSha256']) {
    const copy = fixture(t);
    changeReceipt(copy, (receipt) => { receipt[key] = 'wrong'; });
    assert.throws(() => load(copy), /provenance mismatch/);
  }
});

test('missing local manifest snapshot or missing provenance is rejected', integration, (t) => {
  const copy = fixture(t);
  changeReceipt(copy, (receipt) => { receipt.inputs.pop(); });
  assert.throws(() => load(copy), /input set mismatch/);
  fs.unlinkSync(path.join(copy, 'default.receipt.json'));
  assert.throws(() => load(copy), /ENOENT/);
});

test('modified raw metadata is rejected before policy evaluation', integration, (t) => {
  const copy = fixture(t);
  fs.appendFileSync(path.join(copy, 'default.metadata.json'), '\n');
  assert.throws(() => load(copy), /provenance mismatch/);
});

test('command provenance requires real success and the exact locked offline invocation', integration, (t) => {
  for (const change of [(r) => { r.command.status = 1; }, (r) => { r.command.args = ['metadata']; },
    (r) => { delete r.command.pid; }, (r) => { r.command.startedAt = 'invalid'; }]) {
    const copy = fixture(t);
    changeReceipt(copy, change);
    assert.throws(() => load(copy), /provenance mismatch/);
  }
});

test('reader CLI outputs original bytes and never invokes Cargo on success or failure', integration, (t) => {
  const copy = fixture(t);
  const bin = path.join(copy, 'bin');
  fs.mkdirSync(bin);
  const marker = path.join(copy, 'unexpected-cargo');
  fs.writeFileSync(path.join(bin, 'cargo'), '#!/bin/sh\n: > "$METADATA_CARGO_MARKER"\nexit 91\n', { mode: 0o755 });
  const env = { ...process.env, PATH: `${bin}${path.delimiter}${process.env.PATH}`, METADATA_CARGO_MARKER: marker };
  const cli = path.resolve(__dirname, '../metadata-capture.js');
  const args = [cli, '--read', '--api-dir', apiDir, '--directory', copy];
  const success = spawnSync(process.execPath, args, { env, maxBuffer: 64 * 1024 * 1024 });
  assert.equal(success.status, 0, success.stderr.toString());
  assert.deepEqual(success.stdout, fs.readFileSync(path.join(copy, 'default.metadata.json')));
  fs.unlinkSync(path.join(copy, 'default.receipt.json'));
  const failure = spawnSync(process.execPath, args, { env });
  assert.equal(failure.status, 1);
  assert.equal(failure.stdout.length, 0);
  assert.equal(fs.existsSync(marker), false);
});

const crypto = require('node:crypto');
const { NO_DEPS } = require('../metadata-capture.js');
const hash = (bytes) => crypto.createHash('sha256').update(bytes).digest('hex');

test('no-deps command preserves the original Node Cargo invocation exactly', () => {
  assert.deepEqual(commandArgs('/workspace/api', NO_DEPS), [
    'metadata', '--no-deps', '--offline', '--locked', '--format-version=1',
    '--manifest-path', '/workspace/api/Cargo.toml',
  ]);
  assert.equal(parseArgs(['--read', '--directory', '/captures', '--feature', NO_DEPS]).feature, NO_DEPS);
});

test('genuine no-deps raw graph round-trips with the complete local input closure', integration, () => {
  const raw = fs.readFileSync(path.join(directory, `${NO_DEPS}.metadata.json`));
  assert.deepEqual(load(directory, NO_DEPS), raw);
  const metadata = JSON.parse(raw);
  const resolved = JSON.parse(load(directory));
  const receipt = JSON.parse(fs.readFileSync(path.join(directory, `${NO_DEPS}.receipt.json`)));
  const fullReceipt = JSON.parse(fs.readFileSync(path.join(directory, 'default.receipt.json')));
  assert.equal(metadata.resolve, null);
  assert.deepEqual(metadata.packages.map((pkg) => pkg.id).sort(), [...resolved.workspace_members].sort());
  assert.equal(receipt.localInputGraphSha256, hash(load(directory)));
  assert.deepEqual(receipt.inputs, fullReceipt.inputs);
  const vendor = receipt.inputs.filter((input) => input.path.includes('/vendor/'));
  assert.ok(vendor.length > 0, 'real resolved capture must retain vendor path manifests');
  assert.ok(vendor.some((input) => !metadata.packages.some((pkg) => pkg.manifest_path === input.path)),
    'no-deps must retain its workspace-only semantics');
});

test('no-deps rejects missing capture, receipt, raw graph and input graph', integration, (t) => {
  assert.throws(() => load(path.join(os.tmpdir(), 'absent-no-deps-capture'), NO_DEPS), /ENOENT/);
  for (const file of [`${NO_DEPS}.receipt.json`, `${NO_DEPS}.metadata.json`, 'default.receipt.json', 'default.metadata.json']) {
    const copy = fixture(t);
    fs.unlinkSync(path.join(copy, file));
    assert.throws(() => load(copy, NO_DEPS), /ENOENT/);
  }
});

test('no-deps rejects wrong mode, source, generator, command and linked graph', integration, (t) => {
  for (const change of [(r) => { r.feature = 'default'; }, (r) => { r.sourceSha = 'wrong'; },
    (r) => { r.generatorSha256 = 'wrong'; }, (r) => { r.workspaceRoot = 'wrong'; },
    (r) => { r.command.args = commandArgs(apiDir, 'default'); }, (r) => { r.command.status = 1; },
    (r) => { r.localInputGraphSha256 = 'wrong'; }]) {
    const copy = fixture(t);
    changeReceipt(copy, change, NO_DEPS);
    assert.throws(() => load(copy, NO_DEPS), /provenance mismatch/);
  }
});

test('no-deps rejects stale root, lock, vendor and other local manifest snapshots', integration, (t) => {
  const selectors = [(p) => p === path.join(apiDir, 'Cargo.toml'), (p) => p.endsWith('/Cargo.lock'),
    (p) => p.includes('/vendor/') && p.endsWith('/Cargo.toml'),
    (p) => p.includes('/crates/') && p.endsWith('/Cargo.toml')];
  for (const select of selectors) {
    const copy = fixture(t);
    changeReceipt(copy, (r) => {
      const input = r.inputs.find((item) => select(item.path));
      assert.ok(input);
      input.contentBase64 = Buffer.from('stale input').toString('base64');
    }, NO_DEPS);
    assert.throws(() => load(copy, NO_DEPS), /Stale captured metadata input/);
  }
  const copy = fixture(t);
  changeReceipt(copy, (r) => { r.inputs.pop(); }, NO_DEPS);
  assert.throws(() => load(copy, NO_DEPS), /input set mismatch/);
});

test('no-deps refuses resolved or incomplete workspace graphs even with a matching raw hash', integration, (t) => {
  for (const change of [(m) => { m.resolve = { nodes: [] }; }, (m) => { m.packages.pop(); },
    (m) => { m.workspace_members.pop(); }]) {
    const copy = fixture(t);
    const file = path.join(copy, `${NO_DEPS}.metadata.json`);
    const metadata = JSON.parse(fs.readFileSync(file));
    change(metadata);
    const raw = Buffer.from(JSON.stringify(metadata));
    fs.writeFileSync(file, raw);
    changeReceipt(copy, (r) => { r.metadataSha256 = hash(raw); }, NO_DEPS);
    assert.throws(() => load(copy, NO_DEPS), /does not match the Cargo workspace/);
  }
});

test('configured original Node consumer uses no Cargo and fails closed on missing, wrong or stale no-deps data', integration, (t) => {
  const copy = fixture(t);
  const bin = path.join(copy, 'bin');
  fs.mkdirSync(bin);
  const marker = path.join(copy, 'unexpected-cargo');
  fs.writeFileSync(path.join(bin, 'cargo'), '#!/bin/sh\n: > "$METADATA_CARGO_MARKER"\nexit 91\n', { mode: 0o755 });
  const env = { ...process.env, PATH: `${bin}${path.delimiter}${process.env.PATH}`,
    METADATA_CARGO_MARKER: marker, ONEFLOWBASE_DEPENDENCY_METADATA_DIR: copy };
  // node:test marks worker children; a fresh CLI runner must not inherit that context.
  delete env.NODE_TEST_CONTEXT;
  const target = path.resolve(__dirname, '../../verify/_tests/interface-runtime-boundary.test.js');
  const args = ['--test', '--test-name-pattern=Delivery 1944', target];
  const success = spawnSync(process.execPath, ['--test', target], { env, encoding: 'utf8' });
  assert.equal(success.status, 0, success.stdout + success.stderr);
  for (const value of ['', path.join(copy, 'absent')]) {
    const result = spawnSync(process.execPath, args, {
      env: { ...env, ONEFLOWBASE_DEPENDENCY_METADATA_DIR: value }, encoding: 'utf8',
    });
    assert.equal(result.status, 1, result.stdout + result.stderr);
  }
  const original = fs.readFileSync(path.join(copy, `${NO_DEPS}.receipt.json`));
  for (const change of [(r) => { r.feature = 'default'; }, (r) => {
    r.inputs.find((item) => item.path.includes('/vendor/')).contentBase64 = Buffer.from('stale vendor').toString('base64');
  }]) {
    fs.writeFileSync(path.join(copy, `${NO_DEPS}.receipt.json`), original);
    changeReceipt(copy, change, NO_DEPS);
    const result = spawnSync(process.execPath, args, { env, encoding: 'utf8' });
    assert.equal(result.status, 1, result.stdout + result.stderr);
    assert.match(result.stdout + result.stderr, /provenance mismatch|Stale captured metadata input/);
  }
  fs.unlinkSync(path.join(copy, `${NO_DEPS}.receipt.json`));
  const missing = spawnSync(process.execPath, args, { env, encoding: 'utf8' });
  assert.equal(missing.status, 1, missing.stdout + missing.stderr);
  assert.match(missing.stdout + missing.stderr, /ENOENT/);
  assert.equal(fs.existsSync(marker), false);
});
