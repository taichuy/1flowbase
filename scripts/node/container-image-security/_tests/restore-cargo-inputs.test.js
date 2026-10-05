const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync, spawnSync } = require('node:child_process');
const { restoreCargoInputs } = require('../restore-cargo-inputs');

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cargo-inputs-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  fs.mkdirSync(path.join(root, 'api/src'), { recursive: true });
  const write = (file, content) => fs.writeFileSync(path.join(root, file), content);
  write('api/Cargo.toml', '[package]\nname="cache-fixture"\nversion="0.1.0"\nedition="2021"\n');
  write('api/src/main.rs', 'fn main() { println!("original"); }\n');
  execFileSync('git', ['init', '-q'], { cwd: root });
  execFileSync('git', ['add', 'api'], { cwd: root });
  const manifest = path.join(root, 'target/cargo-inputs.json');
  const restore = () => restoreCargoInputs(root, manifest);
  const touch = file => {
    const timestamp = Date.now() / 1000 + 2;
    fs.utimesSync(path.join(root, file), timestamp, timestamp);
  };
  return { root, write, restore, touch };
}

test('restores only identical files and complete directory contents', t => {
  const f = fixture(t);
  const original = fs.statSync(path.join(f.root, 'api/src/main.rs')).mtimeMs;
  f.restore();
  f.touch('api/src/main.rs');
  f.touch('api/src');
  assert.ok(f.restore().restored >= 3);
  assert.ok(Math.abs(fs.statSync(path.join(f.root, 'api/src/main.rs')).mtimeMs - original) < 0.01);
  f.write('api/src/main.rs', 'changed');
  f.touch('api/src/main.rs');
  const changed = fs.statSync(path.join(f.root, 'api/src/main.rs')).mtimeMs;
  f.restore();
  assert.equal(fs.statSync(path.join(f.root, 'api/src/main.rs')).mtimeMs, changed);
  f.write('api/src/new.rs', 'new input');
  f.touch('api/src');
  const directory = fs.statSync(path.join(f.root, 'api/src')).mtimeMs;
  f.restore();
  assert.equal(fs.statSync(path.join(f.root, 'api/src')).mtimeMs, directory);
  fs.unlinkSync(path.join(f.root, 'api/src/main.rs'));
  assert.doesNotThrow(f.restore);
});

test('real Cargo reuses checkout inputs but rebuilds edits and reverts', t => {
  const f = fixture(t);
  const cargo = () => {
    const result = spawnSync('cargo', ['build', '--release', '--offline', '--manifest-path', 'api/Cargo.toml'], {
      cwd: f.root, encoding: 'utf8', env: { ...process.env, CARGO_TARGET_DIR: path.join(f.root, 'target') },
    });
    assert.equal(result.status, 0, result.stderr);
    return result.stderr;
  };
  const run = () => execFileSync(path.join(f.root, 'target/release/cache-fixture'), { encoding: 'utf8' }).trim();
  f.restore();
  assert.match(cargo(), /Compiling cache-fixture/);
  // Reproduce a fresh checkout without sleeping or changing content.
  f.touch('api/src/main.rs');
  assert.match(cargo(), /Compiling cache-fixture/, 'mtime-only checkout is the regression');
  // Establish a new successful cache at realistic (non-future) timestamps.
  const now = Date.now() / 1000 - 1;
  fs.utimesSync(path.join(f.root, 'api/src/main.rs'), now, now);
  f.restore();
  cargo();
  f.touch('api/src/main.rs');
  f.restore();
  assert.doesNotMatch(cargo(), /Compiling cache-fixture/);
  for (const value of ['edited', 'original']) {
    f.write('api/src/main.rs', `fn main() { println!("${value}"); }\n`);
    f.touch('api/src/main.rs');
    f.restore();
    assert.match(cargo(), /Compiling cache-fixture/);
    assert.equal(run(), value);
  }
});

test('directory-watching build scripts rebuild for added and deleted inputs', t => {
  const f = fixture(t);
  fs.mkdirSync(path.join(f.root, 'api/migrations'));
  f.write('api/migrations/one.sql', 'select 1;');
  f.write('api/build.rs', 'fn main() { println!("cargo:rerun-if-changed=migrations"); println!("cargo:rustc-env=INPUT_COUNT={}", std::fs::read_dir("migrations").unwrap().count()); }');
  f.write('api/src/main.rs', 'fn main() { println!("{}", env!("INPUT_COUNT")); }');
  execFileSync('git', ['add', 'api'], { cwd: f.root });
  const cargo = () => {
    const result = spawnSync('cargo', ['build', '--release', '--offline', '--manifest-path', 'api/Cargo.toml'], {
      cwd: f.root, encoding: 'utf8', env: { ...process.env, CARGO_TARGET_DIR: path.join(f.root, 'target') },
    });
    assert.equal(result.status, 0, result.stderr);
    return result.stderr;
  };
  const run = () => execFileSync(path.join(f.root, 'target/release/cache-fixture'), { encoding: 'utf8' }).trim();
  f.restore();
  cargo();
  f.touch('api/migrations/one.sql');
  f.touch('api/migrations');
  f.restore();
  assert.doesNotMatch(cargo(), /Compiling cache-fixture/);
  assert.equal(run(), '1');
  f.write('api/migrations/two.sql', 'select 2;');
  f.touch('api/migrations');
  execFileSync('git', ['add', 'api'], { cwd: f.root });
  f.restore();
  assert.match(cargo(), /Compiling cache-fixture/);
  assert.equal(run(), '2');
  fs.unlinkSync(path.join(f.root, 'api/migrations/one.sql'));
  f.touch('api/migrations');
  f.restore();
  assert.match(cargo(), /Compiling cache-fixture/);
  assert.equal(run(), '1');
});
