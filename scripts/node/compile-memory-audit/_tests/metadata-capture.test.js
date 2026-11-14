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
