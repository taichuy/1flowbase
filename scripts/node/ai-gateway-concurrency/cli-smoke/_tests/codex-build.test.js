'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { parseArgs } = require('../../../cli/ai-gateway-codex-build');
const {
  parseChecksumManifest,
  planCodexBuild,
  runCodexBuild,
} = require('../codex-build');

function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'codex-low-memory-build-'));
  const sourceRoot = path.join(root, 'codex');
  const outputRoot = path.join(root, 'artifacts');
  fs.mkdirSync(path.join(sourceRoot, '.github', 'scripts'), { recursive: true });
  fs.mkdirSync(path.join(sourceRoot, 'codex-rs'), { recursive: true });
  fs.writeFileSync(path.join(sourceRoot, '.github', 'scripts', 'rusty_v8_bazel.py'), '# fixture\n');
  fs.writeFileSync(path.join(sourceRoot, 'codex-rs', 'Cargo.toml'), '[workspace]\n');
  return { root, sourceRoot, outputRoot };
}

test('CLI arguments require source/output and default to two jobs', () => {
  assert.deepEqual(parseArgs(['--source-root', '/src/codex', '--output-dir', '/tmp/evidence']), {
    sourceRoot: '/src/codex', outputDir: '/tmp/evidence', jobs: 2,
  });
  assert.equal(parseArgs([
    '--source-root', '/src/codex', '--output-dir', '/tmp/evidence', '--jobs', '3',
  ]).jobs, 3);
  assert.equal(parseArgs([
    '--source-root', '/src/codex', '--output-dir', '/tmp/evidence',
    '--rusty-v8-dir', '/home/user/Downloads',
  ]).rustyV8Dir, '/home/user/Downloads');
  assert.throws(() => parseArgs(['--source-root', '/src/codex']), /--output-dir/u);
  assert.throws(() => parseArgs([
    '--source-root', '/src/codex', '--output-dir', '/tmp/evidence', '--jobs', '0',
  ]), /positive integer/u);
});

function writeProvidedArtifacts(directory, { tamper = false, omit = null } = {}) {
  const target = 'x86_64-unknown-linux-gnu';
  const names = {
    archive: `librusty_v8_release_${target}.a.gz`,
    binding: `src_binding_release_${target}.rs`,
    checksums: `rusty_v8_release_${target}.sha256`,
  };
  fs.mkdirSync(directory, { recursive: true });
  const archive = Buffer.from('provided verified archive');
  const binding = Buffer.from(tamper ? 'tampered binding' : 'provided verified binding');
  const expectedBinding = Buffer.from('provided verified binding');
  if (omit !== 'archive') fs.writeFileSync(path.join(directory, names.archive), archive);
  if (omit !== 'binding') fs.writeFileSync(path.join(directory, names.binding), binding);
  if (omit !== 'checksums') fs.writeFileSync(path.join(directory, names.checksums), [
    `${crypto.createHash('sha256').update(archive).digest('hex')}  ${names.archive}`,
    `${crypto.createHash('sha256').update(expectedBinding).digest('hex')}  ${names.binding}`,
    '',
  ].join('\n'));
  return names;
}

function localArtifactDependencies(files, calls) {
  return {
    inspectSource: () => ({
      source: {
        fixed_revision: '56395bddaf26eb2829387ca6a417bf9128e5b239',
        observed_revision: '56395bddaf26eb2829387ca6a417bf9128e5b239', dirty: false, detached: true,
        identity: 'github:openai/codex', observed_remote: 'https://github.com/openai/codex.git',
      },
    }),
    runCommand(executable, args, options) {
      calls.push({ executable, args, options });
      if (executable === 'python3') return { status: 0, stdout: '149.2.0\n', stderr: '' };
      if (executable === 'rustc') return { status: 0, stdout: 'host: x86_64-unknown-linux-gnu\n', stderr: '' };
      if (executable === 'gzip' || executable === 'sha256sum') {
        return { status: 0, stdout: 'OK\n', stderr: '' };
      }
      if (executable === 'cargo') {
        const binary = path.join(files.sourceRoot, 'codex-rs', 'target', 'debug', 'codex');
        fs.mkdirSync(path.dirname(binary), { recursive: true });
        fs.writeFileSync(binary, '#!/bin/sh\n', { mode: 0o700 });
        return { status: 0, stdout: '', stderr: '' };
      }
      throw new Error(`unexpected command ${executable}`);
    },
  };
}

test('verified local official artifacts are copied, reverified, and used without curl', () => {
  const files = fixture();
  const provided = path.join(files.root, 'Downloads');
  const names = writeProvidedArtifacts(provided);
  const calls = [];
  try {
    const evidence = runCodexBuild({
      sourceRoot: files.sourceRoot, outputDir: files.outputRoot, jobs: 2, rustyV8Dir: provided,
    }, localArtifactDependencies(files, calls));
    assert.equal(evidence.rusty_v8.acquisition.mode, 'verified-local-official-artifacts');
    assert.equal(evidence.rusty_v8.urls.length, 3);
    assert.deepEqual(Object.keys(evidence.rusty_v8.acquisition.provided_artifacts), [
      'archive', 'binding', 'checksums',
    ]);
    assert.equal(calls.some((call) => call.executable === 'curl'), false);
    assert.equal(calls.filter((call) => call.executable === 'gzip').length, 2);
    assert.equal(calls.filter((call) => call.executable === 'sha256sum').length, 2);
    for (const name of Object.values(names)) {
      assert.equal(fs.existsSync(path.join(files.outputRoot, 'rusty_v8', name)), true);
    }
  } finally {
    fs.rmSync(files.root, { recursive: true, force: true });
  }
});

test('local official artifact mode fails closed when a required file is missing', () => {
  const files = fixture();
  const provided = path.join(files.root, 'Downloads');
  writeProvidedArtifacts(provided, { omit: 'binding' });
  const calls = [];
  try {
    assert.throws(() => runCodexBuild({
      sourceRoot: files.sourceRoot, outputDir: files.outputRoot, jobs: 2, rustyV8Dir: provided,
    }, localArtifactDependencies(files, calls)), /missing official rusty_v8 artifact/u);
    assert.equal(calls.some((call) => ['curl', 'cargo'].includes(call.executable)), false);
  } finally {
    fs.rmSync(files.root, { recursive: true, force: true });
  }
});

test('local official artifact mode rejects a checksum-valid-name tamper before copying or build', () => {
  const files = fixture();
  const provided = path.join(files.root, 'Downloads');
  writeProvidedArtifacts(provided, { tamper: true });
  const calls = [];
  try {
    assert.throws(() => runCodexBuild({
      sourceRoot: files.sourceRoot, outputDir: files.outputRoot, jobs: 2, rustyV8Dir: provided,
    }, localArtifactDependencies(files, calls)), /checksum mismatch/u);
    assert.equal(calls.some((call) => ['curl', 'cargo'].includes(call.executable)), false);
  } finally {
    fs.rmSync(files.root, { recursive: true, force: true });
  }
});

test('build plan mirrors the pinned official rusty_v8 release action without release or LTO', () => {
  const value = planCodexBuild('/src/codex', '/tmp/evidence', '149.2.0', 'x86_64-unknown-linux-gnu', 2);
  assert.deepEqual(value.urls, [
    'https://github.com/openai/codex/releases/download/rusty-v8-v149.2.0/librusty_v8_release_x86_64-unknown-linux-gnu.a.gz',
    'https://github.com/openai/codex/releases/download/rusty-v8-v149.2.0/src_binding_release_x86_64-unknown-linux-gnu.rs',
    'https://github.com/openai/codex/releases/download/rusty-v8-v149.2.0/rusty_v8_release_x86_64-unknown-linux-gnu.sha256',
  ]);
  assert.deepEqual(value.build.args, [
    'build', '--locked', '--manifest-path', 'codex-rs/Cargo.toml',
    '--bin', 'codex', '--jobs', '2',
  ]);
  assert.equal(value.build.args.includes('--release'), false);
  assert.equal(value.environment.RUSTY_V8_ARCHIVE, value.paths.archive);
  assert.equal(value.environment.RUSTY_V8_SRC_BINDING_PATH, value.paths.binding);
});

test('checksum parser requires exactly the official archive and binding rows', () => {
  const archive = 'a'.repeat(64);
  const binding = 'b'.repeat(64);
  assert.deepEqual(parseChecksumManifest(
    `${archive}  librusty_v8_release_x86_64-unknown-linux-gnu.a.gz\n`
      + `${binding}  src_binding_release_x86_64-unknown-linux-gnu.rs\n`,
    'x86_64-unknown-linux-gnu'
  ), [
    { sha256: archive, file: 'librusty_v8_release_x86_64-unknown-linux-gnu.a.gz' },
    { sha256: binding, file: 'src_binding_release_x86_64-unknown-linux-gnu.rs' },
  ]);
  assert.throws(() => parseChecksumManifest(`${archive}  one-file\n`, 'x86_64-unknown-linux-gnu'), /exactly two/u);
  assert.throws(() => parseChecksumManifest(
    `${archive}  first\n${binding}  second\n\n`, 'x86_64-unknown-linux-gnu'
  ), /exactly two/u);
  assert.throws(() => parseChecksumManifest(
    `${archive}  wrong.a.gz\n${binding}  wrong.rs\n`, 'x86_64-unknown-linux-gnu'
  ), /official rusty_v8 files/u);
});

test('runner verifies official artifacts, performs a debug build, and writes reproducible evidence', () => {
  const files = fixture();
  const calls = [];
  const archiveBytes = Buffer.from('verified archive');
  const bindingBytes = Buffer.from('verified binding');
  const digest = (value) => crypto.createHash('sha256').update(value).digest('hex');
  try {
    const evidence = runCodexBuild({
      sourceRoot: files.sourceRoot, outputDir: files.outputRoot, jobs: 2,
    }, {
      inspectSource: () => ({
        source: {
          fixed_revision: '56395bddaf26eb2829387ca6a417bf9128e5b239',
          observed_revision: '56395bddaf26eb2829387ca6a417bf9128e5b239', dirty: false, detached: true,
          identity: 'github:openai/codex', observed_remote: 'https://github.com/openai/codex.git',
        },
      }),
      runCommand(executable, args, options) {
        calls.push({ executable, args, options });
        if (executable === 'python3') return { status: 0, stdout: '149.2.0\n', stderr: '' };
        if (executable === 'rustc') return { status: 0, stdout: 'rustc 1.93.0\nhost: x86_64-unknown-linux-gnu\n', stderr: '' };
        if (executable === 'curl') {
          const output = args[args.indexOf('--output') + 1];
          if (output.endsWith('.a.gz')) fs.writeFileSync(output, archiveBytes);
          else if (output.endsWith('.rs')) fs.writeFileSync(output, bindingBytes);
          else fs.writeFileSync(output,
            `${digest(archiveBytes)}  librusty_v8_release_x86_64-unknown-linux-gnu.a.gz\n`
            + `${digest(bindingBytes)}  src_binding_release_x86_64-unknown-linux-gnu.rs\n`);
          return { status: 0, stdout: '', stderr: '' };
        }
        if (executable === 'sha256sum') return { status: 0, stdout: 'OK\n', stderr: '' };
        if (executable === 'cargo') {
          const binary = path.join(files.sourceRoot, 'codex-rs', 'target', 'debug', 'codex');
          fs.mkdirSync(path.dirname(binary), { recursive: true });
          fs.writeFileSync(binary, '#!/bin/sh\n', { mode: 0o700 });
          return { status: 0, stdout: '', stderr: '' };
        }
        throw new Error(`unexpected command ${executable}`);
      },
    });
    assert.equal(evidence.status, 'pass');
    assert.equal(evidence.source.observed_revision, '56395bddaf26eb2829387ca6a417bf9128e5b239');
    assert.equal(evidence.build.profile, 'debug');
    assert.equal(evidence.build.jobs, 2);
    assert.equal(evidence.build.exit_code, 0);
    assert.equal(evidence.rusty_v8.urls.length, 3);
    assert.equal(evidence.rusty_v8.checksums.length, 2);
    assert.match(evidence.executable.sha256, /^[a-f0-9]{64}$/u);
    assert.equal(fs.existsSync(path.join(files.outputRoot, 'codex-build-provenance.json')), true);
    const curlCalls = calls.filter((call) => call.executable === 'curl');
    assert.equal(curlCalls.length, 3);
    for (const call of curlCalls) {
      for (const flag of ['--fail', '--retry', '--retry-all-errors', '--output']) assert.ok(call.args.includes(flag));
    }
    const cargo = calls.find((call) => call.executable === 'cargo');
    assert.equal(cargo.args.includes('--release'), false);
    assert.equal(cargo.options.env.RUSTY_V8_ARCHIVE.endsWith('.a.gz'), true);
    assert.equal(cargo.options.env.RUSTY_V8_SRC_BINDING_PATH.endsWith('.rs'), true);
    assert.equal(calls.some((call) => ['brew', 'npm'].includes(call.executable)), false);
  } finally {
    fs.rmSync(files.root, { recursive: true, force: true });
  }
});

test('download failure stops before checksum, build, or fallback commands', () => {
  const files = fixture();
  const calls = [];
  try {
    assert.throws(() => runCodexBuild({
      sourceRoot: files.sourceRoot, outputDir: files.outputRoot, jobs: 2,
    }, {
      inspectSource: () => ({ source: { observed_revision: 'fixed', dirty: false, detached: true } }),
      runCommand(executable, args) {
        calls.push(executable);
        if (executable === 'python3') return { status: 0, stdout: '149.2.0\n', stderr: '' };
        if (executable === 'rustc') return { status: 0, stdout: 'host: x86_64-unknown-linux-gnu\n', stderr: '' };
        if (executable === 'curl') return { status: 22, stdout: '', stderr: 'HTTP 404' };
        throw new Error(`must not run ${executable}`);
      },
    }), /curl.*22/u);
    assert.deepEqual(calls, ['python3', 'rustc', 'curl']);
  } finally {
    fs.rmSync(files.root, { recursive: true, force: true });
  }
});
