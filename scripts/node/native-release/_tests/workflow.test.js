const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '../../../..');
const read = name => fs.readFileSync(path.join(root, '.github/workflows', name), 'utf8').replace(/\r\n/g, '\n');
function job(text, name) {
  const start = text.indexOf(`\n  ${name}:\n`);
  assert.ok(start >= 0, name);
  const rest = text.slice(start + 1);
  const end = rest.slice(1).search(/\n  [a-z][\w-]*:\n/);
  return end < 0 ? rest : rest.slice(0, end + 1);
}
test('Docker release and manual native release share the same build-only workflow and candidate context', () => {
  const docker = read('container-images.yml');
  const manual = read('native-release.yml');
  const build = read('native-build.yml');
  assert.match(build, /on:\n  workflow_call:/);
  assert.ok(!build.includes('\n  release:\n'));
  assert.ok(!build.includes('contents: write'));
  assert.ok(!build.includes('ref: ${{'));
  assert.match(job(docker, 'build-native'), /uses: \.\/\.github\/workflows\/native-build.yml/);
  assert.match(job(docker, 'build-native'), /target: all/);
  assert.match(job(docker, 'build-native'), /release_tag: \$\{\{ needs.select-release.outputs.tag \}\}/);
  assert.match(job(manual, 'build'), /uses: \.\/\.github\/workflows\/native-build.yml/);
  assert.match(job(manual, 'build'), /release_tag: \$\{\{ inputs.release_tag \}\}/);
  for (const target of ['linux-x64', 'linux-arm64', 'macos-x64', 'macos-arm64', 'windows-x64']) assert.ok(build.includes(`target:'${target}'`));
  for (const name of ['build-web-dist', 'build-api-server-binary']) assert.match(job(docker, name), /- select-release/);
});
test('one publisher waits for both distributions and consumes native artifacts from the same run', () => {
  const publication = job(read('container-images.yml'), 'publish-release');
  for (const dependency of ['publish-web', 'publish-api-server', 'build-native']) {
    assert.ok(publication.includes(`- ${dependency}`));
    assert.ok(publication.includes(`needs.${dependency}.result == 'success'`));
  }
  assert.match(publication, /needs.select-release.outputs.enabled == 'true'/);
  assert.match(publication, /NATIVE_SHA: \$\{\{ github.sha \}\}/);
  assert.match(publication, /NATIVE_RELEASE_KIND: combined/);
  assert.match(publication, /pattern: 1flowbase-\*/);
  assert.ok(!publication.includes('run-id:'));
  assert.match(job(read('native-release.yml'), 'release'), /needs: build/);
});

test('the actual release condition blocks failed builds whose publish jobs were skipped', () => {
  const publication = job(read('container-images.yml'), 'publish-release');
  const expression = publication.match(/    if: \$\{\{ (.+) \}\}/)[1]
    .replace(/needs\.([\w-]+)/g, 'needs["$1"]');
  // Evaluate this workflow's small expression vocabulary, not a duplicate implementation of its gate.
  const gate = new Function('needs', 'github', 'always', 'inputs', `return (${expression});`);
  const baseline = () => ({
    'select-web': { result: 'success', outputs: { enabled: 'true' } },
    'select-api-server': { result: 'success', outputs: { enabled: 'true' } },
    'select-release': { result: 'success', outputs: { enabled: 'true' } },
    'publish-web': { result: 'success' }, 'publish-api-server': { result: 'success' }, 'build-native': { result: 'success' },
  });
  const github = { event_name: 'push', ref: 'refs/heads/latest' };
  assert.equal(gate(baseline(), github, () => true), true);
  for (const component of ['web', 'api-server']) {
    const needs = baseline();
    needs[`publish-${component}`].result = 'skipped';
    assert.equal(gate(needs, github, () => true), false, `${component} build failed => publisher skipped`);
    needs[`select-${component}`].outputs.enabled = 'false';
    assert.equal(gate(needs, github, () => true), true, `${component} was intentionally disabled`);
  }
  for (const dependency of ['publish-web', 'publish-api-server', 'build-native', 'select-release']) {
    const needs = baseline(); needs[dependency].result = 'failure';
    assert.equal(gate(needs, github, () => true), false, dependency);
  }
  assert.equal(gate(baseline(), { ...github, ref: 'refs/heads/dev' }, () => true), false);
  const manual = { ...github, event_name: 'workflow_dispatch' };
  assert.equal(gate(baseline(), manual, () => true, { component: 'all', promote_official_tags: true }), true);
  assert.equal(gate(baseline(), manual, () => true, { component: 'all', promote_official_tags: false }), false);
  for (const component of ['web', 'api-server']) {
    assert.equal(gate(baseline(), manual, () => true, { component, promote_official_tags: true }), false);
  }
  assert.equal(gate(baseline(), { ...manual, ref: 'refs/heads/beta' }, () => true, { component: 'all', promote_official_tags: true }), false);
});

test('manual complete publication enables both selectors and supplies an explicit planner flag', () => {
  const text = read('container-images.yml');
  assert.match(text, /options:\n\s+- all\n/);
  for (const name of ['select-web', 'select-api-server']) {
    assert.match(job(text, name), /RELEASE_COMPONENT.*== "all"/);
  }
  assert.match(job(text, 'select-release'), /RELEASE_MANUAL_COMBINED: \$\{\{ github.event_name == 'workflow_dispatch' && inputs.component == 'all' \}\}/);
  assert.match(job(text, 'select-release'), /RELEASE_PROMOTE_OFFICIAL_TAGS: \$\{\{ inputs.promote_official_tags \}\}/);
});
test('publishers serialize the same tag across workflows without cancelling active publication', () => {
  const docker = read('container-images.yml');
  const combined = job(docker, 'publish-release');
  const native = job(read('native-release.yml'), 'release');
  const normalizedGroup = text => text.match(/      group: (.+)/)[1]
    .replace('${{ github.repository }}', 'taichuy/1flowbase')
    .replace(/\$\{\{ (needs.select-release.outputs.tag|inputs.release_tag) \}\}/g, 'v0.5.9');
  assert.equal(normalizedGroup(combined), normalizedGroup(native));
  for (const text of [combined, native]) assert.match(text, /      cancel-in-progress: false/);
  assert.match(docker.slice(0, docker.indexOf('jobs:')), /  cancel-in-progress: false/);
});

test('Docker and native packages consume one web build from the same run', () => {
  const docker = read('container-images.yml');
  const manual = read('native-release.yml');
  const native = read('native-build.yml');
  const web = read('web-build.yml');
  for (const caller of [docker, manual]) {
    const build = job(caller, 'build-web-dist');
    assert.match(build, /uses: \.\/\.github\/workflows\/web-build.yml/);
    assert.doesNotMatch(build, /matrix:/);
    assert.match(caller === docker ? job(caller, 'build-native') : job(caller, 'build'), /- build-web-dist/);
  }
  assert.match(job(docker, 'build-web-dist'), /needs\.select-web\.outputs\.enabled == 'true' \|\| needs\.select-release\.outputs\.enabled == 'true'/);
  assert.match(web, /cache: pnpm/);
  assert.match(web, /cache-dependency-path: web\/pnpm-lock.yaml/);
  assert.equal((web.match(/pnpm --dir web --filter @1flowbase\/web build/g) || []).length, 1);
  assert.match(web, /name: web-dist\n/);
  assert.doesNotMatch(native, /pnpm|name: Build web/);
  assert.match(native, /name: web-dist\n\s+path: tmp\/shared-web/);
  assert.match(native, /--web-dir tmp\/shared-web\/dist/);
  assert.match(native, /node scripts\/node\/native-release\/verify-web.js tmp\/shared-web/);
  assert.doesNotMatch(native, /sha256sum/);
  const publish = job(docker, 'publish-web');
  assert.match(publish, /name: web-dist\n\s+path: tmp\/shared-web/);
  for (const arch of ['amd64', 'arm64']) {
    assert.ok(publish.includes(`cp -a tmp/shared-web/dist tmp/web-dist/${arch}/dist`));
  }
  assert.doesNotMatch(docker, /pnpm --dir web .*build|pattern: web-dist-\*/);
});

test('shared web selection includes native releases and manual web rebuilds only', () => {
  const build = job(read('container-images.yml'), 'build-web-dist');
  const expression = build.match(/    if: (.+)/)[1].replace(/needs\.([\w-]+)/g, 'needs["$1"]');
  const enabled = new Function('needs', `return (${expression});`);
  for (const [web, release, expected] of [
    ['true', 'true', true], ['false', 'true', true],
    ['true', 'false', true], ['false', 'false', false],
  ]) {
    assert.equal(enabled({
      'select-web': { outputs: { enabled: web } },
      'select-release': { outputs: { enabled: release } },
    }), expected);
  }
});

test('native release targets reuse compatible snapshots without depending on unrelated installed toolchains', () => {
  const native = read('native-build.yml');
  assert.match(native, /RUSTUP_TOOLCHAIN: '1\.99\.0'/);
  assert.doesNotMatch(native, /Swatinem\/rust-cache|rustup toolchain list/);
  assert.match(native, /rustc.*-vV/);
  const block = native.split('      - name: Restore native release target\n')[1]
    .split('      - name: Restore native checkout timestamps')[0];
  const primary = block.match(/^          key: (.+)$/m)[1];
  const prefixes = block.split('          restore-keys: |\n')[1].trim().split('\n').map(line => line.trim());
  const render = (text, overrides = {}) => text.replace(/\$\{\{ (.*?) \}\}/g, (_, key) => ({
    'matrix.target': 'macos-arm64', 'steps.compiler.outputs.fingerprint': 'compiler-a',
    "hashFiles('scripts/node/container-image-security/restore-cargo-inputs.js', '.cargo/config*', 'api/.cargo/config*')": 'recipe-a',
    "hashFiles('api/Cargo.lock', 'api/**/Cargo.toml', 'api/rust-toolchain.toml')": 'deps-a',
    'github.sha': 'commit-a', ...overrides,
  })[key] ?? assert.fail(`Unknown cache input ${key}`));
  const previous = render(primary);
  assert.notEqual(previous, render(primary, { 'github.sha': 'commit-b' }));
  assert.ok(previous.startsWith(render(prefixes[0], { 'github.sha': 'commit-b' })));
  for (const changes of [
    { 'matrix.target': 'macos-x64' },
    { 'steps.compiler.outputs.fingerprint': 'compiler-b' },
    { "hashFiles('scripts/node/container-image-security/restore-cargo-inputs.js', '.cargo/config*', 'api/.cargo/config*')": 'recipe-b' },
  ]) assert.ok(prefixes.every(prefix => !previous.startsWith(render(prefix, changes))));
  assert.match(native, /node scripts\/node\/container-image-security\/restore-cargo-inputs.js api\/target\/cargo-inputs.json/);
  assert.match(native, /name: Save native release target\n\s+if: steps\.backend\.outcome == 'success' && steps\.release_target\.outputs\.cache-hit != 'true'/);
  assert.match(native, /native_distribution --timings/);
  assert.match(native, /test-cargo-timing.html/);
  assert.match(native, /build-cargo-timing.html/);
});

test('the actual native compiler fingerprint tracks compilers, SDK and flags rather than unrelated Runner software', () => {
  const { runInNewContext } = require('node:vm');
  const block = read('native-build.yml').split('      - name: Identify native compiler and build environment\n')[1]
    .split('      - name: Restore native Cargo downloads')[0];
  const script = block.split("node <<'NODE'\n")[1].split('\n          NODE')[0].replace(/^          /gm, '');
  const evaluate = (env = {}, overrides = {}) => {
    let receipt;
    const values = { rustc: 'rustc 1.99.0 host=aarch64-apple-darwin', cargo: 'cargo 1.99.0', clang: 'Apple clang 17', sdk: '15.7', ...overrides };
    runInNewContext(script, {
      process: { platform: 'darwin', env }, console: { log() {} },
      require(name) {
        if (name === 'node:fs') return { mkdirSync() {}, appendFileSync() {}, writeFileSync(file, text) { receipt = JSON.parse(text); } };
        if (name === 'node:os') return { availableParallelism: () => 3, totalmem: () => 7 * 1024 ** 3 };
        if (name === 'node:child_process') return { execFileSync(command, args) {
          if (command === 'rustc') return values.rustc;
          if (command === 'cargo') return values.cargo;
          if (command === 'xcrun') return args[0] === 'clang' ? values.clang : values.sdk;
          assert.fail(`Unexpected compiler probe: ${command}`);
        } };
        return require(name);
      },
    });
    return receipt.fingerprint;
  };
  const baseline = evaluate();
  assert.equal(evaluate({ ImageVersion: 'new-image', UNRELATED_RUST_TOOLCHAIN: '1.100' }), baseline);
  for (const env of [{ RUSTFLAGS: '-Ctarget-cpu=native' }, { CARGO_PROFILE_RELEASE_OPT_LEVEL: '2' }, { MACOSX_DEPLOYMENT_TARGET: '14.0' }]) {
    assert.notEqual(evaluate(env), baseline);
  }
  for (const changed of [{ rustc: 'rustc 1.100.0' }, { clang: 'Apple clang 18' }, { sdk: '16.0' }]) {
    assert.notEqual(evaluate({}, changed), baseline);
  }
});
