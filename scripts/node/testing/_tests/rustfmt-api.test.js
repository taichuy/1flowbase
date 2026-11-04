const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { apiFormatInputs, runRustFormat } = require('../rustfmt-api.js');

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'api-rustfmt-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const write = (name, text) => {
    const file = path.join(root, name);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, text);
    return file;
  };
  write('api/Cargo.toml', '[workspace.package]\nedition = "2021"\n[workspace.dependencies]\n');
  const files = [
    write('api/apps/api-server/src/lib_root.rs', 'include!("lib.rs");\n'),
    write('api/apps/api-server/src/lib.rs', 'include!("shared.rs");\n'),
    write('api/apps/api-server/src/shared.rs', 'pub fn shared() {}\n'),
    write('api/apps/api-server/src/routes/mod.rs', 'macro_rules! mount { () => { #[path = "_tests/leaf.rs"] mod tests; } }\nmount!();\n'),
    write('api/apps/api-server/src/routes/_tests/leaf.rs', '#[test] fn leaf() {}\n'),
    write('api/apps/api-server/tests/native.rs', '#[test] fn native() {}\n'),
  ];
  return { root, files, write };
}

test('format retains workspace Cargo coverage and explicitly includes shared roots and macro-mounted leaves', (t) => {
  const f = fixture(t);
  const calls = [];
  assert.equal(runRustFormat({ repoRoot: f.root, env: { FORMAT_FIXTURE: '1' }, spawnSyncImpl(command, args, options) {
    calls.push({ command, args, options });
    return { status: 0 };
  } }), 0);
  assert.deepEqual(calls.map(({ command }) => command), ['cargo', 'rustfmt']);
  assert.deepEqual(calls[0].args, ['fmt', '--all', '--', '--check']);
  assert.deepEqual(calls[1].args, ['--check', '--edition', '2021', ...f.files.sort()]);
  for (const call of calls) {
    assert.equal(call.options.cwd, path.join(f.root, 'api'));
    assert.deepEqual(call.options.env, { FORMAT_FIXTURE: '1' });
  }
});

test('either formatter failure propagates and prevents later stages', (t) => {
  const f = fixture(t);
  for (const failedStage of [0, 1]) {
    let calls = 0;
    assert.equal(runRustFormat({ repoRoot: f.root, spawnSyncImpl() {
      return { status: calls++ === failedStage ? 17 : 0 };
    } }), 17);
    assert.equal(calls, failedStage + 1);
  }
  const error = new Error('formatter missing');
  assert.throws(() => runRustFormat({ repoRoot: f.root, spawnSyncImpl: () => ({ error, status: null }) }), (value) => value === error);
  assert.equal(runRustFormat({ repoRoot: f.root, spawnSyncImpl: () => ({ status: null, signal: 'SIGTERM' }) }), 1);
});

test('missing or empty source roots fail before running any formatter', (t) => {
  const f = fixture(t);
  for (const name of ['src', 'tests']) {
    const root = path.join(f.root, 'api/apps/api-server', name);
    const saved = root + '-saved';
    fs.renameSync(root, saved);
    const options = { repoRoot: f.root, spawnSyncImpl: () => assert.fail('must fail before spawning') };
    assert.throws(() => runRustFormat(options), /Missing API Rust format source directory/);
    fs.mkdirSync(root);
    assert.throws(() => runRustFormat(options), /Empty API Rust format source directory/);
    fs.rmdirSync(root);
    fs.renameSync(saved, root);
  }
});

test('edition and explicit root must match the trusted workspace shape', (t) => {
  const f = fixture(t);
  assert.throws(() => apiFormatInputs(), /explicit absolute/);
  assert.throws(() => apiFormatInputs('relative'), /explicit absolute/);
  for (const manifest of [
    '[workspace.package]\nedition = "2024"\n',
    '[package]\nedition = "2021"\n',
    '[workspace.package]\nedition = "2021"\nedition = "2021"\n',
  ]) {
    f.write('api/Cargo.toml', manifest);
    assert.throws(() => runRustFormat({ repoRoot: f.root, spawnSyncImpl: () => assert.fail('must fail before spawning') }), /declared workspace/);
  }
});
