'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const {
  FIXED_SOURCE_SHA,
  pinnedClaudeProvenance,
  sourceBuiltProvenance,
} = require('../provenance');

// D3-AC-007: the fixed source identity and dirty=false gate are evidence, not labels.
test('source-built provenance fixes source SHA, lock/toolchain digests, command, and executable', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'd3-provenance-'));
  try {
    const executable = path.join(root, 'codex');
    fs.writeFileSync(executable, 'fixed binary fixture');
    fs.writeFileSync(path.join(root, 'Cargo.lock'), 'fixed lock fixture');
    fs.writeFileSync(path.join(root, 'rust-toolchain.toml'), '[toolchain]\nchannel = "1.88"\n');
    const cleanGit = (_cwd, args) => args[0] === 'rev-parse' ? FIXED_SOURCE_SHA.codex : '';
    const value = sourceBuiltProvenance('codex', executable, {
      sourceRoot: root,
      sourceIdentity: 'github:openai/codex',
      buildCommand: 'cargo build --release --locked',
    }, { git: cleanGit });
    assert.equal(value.source.observed_revision, FIXED_SOURCE_SHA.codex);
    assert.equal(value.source.dirty, false);
    assert.equal(value.provenance_claim, 'source-built-from-fixed-git-commit');
    assert.deepEqual(value.toolchain_and_lockfiles.map((entry) => entry.name), [
      'rust-toolchain.toml', 'Cargo.lock',
    ]);
    assert.match(value.executable.sha256, /^[a-f0-9]{64}$/u);
    assert.throws(() => sourceBuiltProvenance('codex', executable, {
      sourceRoot: root, sourceIdentity: 'github:openai/codex',
      buildCommand: 'cargo build --release --locked',
    }, { git: (_cwd, args) => args[0] === 'rev-parse' ? FIXED_SOURCE_SHA.codex : ' M Cargo.lock' }), /clean/u);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('Claude provenance is only a configurable pinned-package binary claim', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'd3-claude-provenance-'));
  try {
    const executable = path.join(root, 'claude');
    fs.writeFileSync(executable, 'fixed package binary fixture');
    const value = pinnedClaudeProvenance(executable, {
      packageName: '@anthropic-ai/claude-code',
      packageVersion: 'configured-version',
      packageIntegrity: 'sha512-configured-integrity',
      installCommand: 'npm install --global @anthropic-ai/claude-code@configured-version',
    });
    assert.equal(value.provenance_claim, 'pinned-package-binary');
    assert.equal(value.source.kind, 'package');
    assert.equal(value.source.dirty, null);
    assert.equal(value.package.version, 'configured-version');
    assert.equal(value.source.fixed_revision, undefined);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
