const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { spawnSync, execFileSync } = require('node:child_process');

const repoRoot = path.resolve(__dirname, '../../../..');
const actionDir = path.join(repoRoot, '.github/actions/setup-trivy');
const action = fs.readFileSync(path.join(actionDir, 'action.yml'), 'utf8');
const version = action.match(/default: (v\d+\.\d+\.\d+)/)[1];

function fixture(t, { arch = 'x86_64', checksum = 'valid', curlFailure = '' } = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'trivy setup '));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const bin = path.join(root, 'bin');
  const assets = path.join(root, 'assets');
  const source = path.join(root, 'source');
  for (const dir of [bin, assets, source]) fs.mkdirSync(dir);
  const githubPath = path.join(root, 'github-path');
  fs.writeFileSync(githubPath, '');
  fs.writeFileSync(path.join(source, 'trivy'), `#!/bin/sh\necho 'Version: ${version.slice(1)}'\n`, { mode: 0o755 });
  const asset = `trivy_${version.slice(1)}_Linux-${arch === 'x86_64' ? '64bit' : 'ARM64'}.tar.gz`;
  execFileSync('tar', ['-czf', path.join(assets, asset), '-C', source, 'trivy']);
  const hash = createHash('sha256').update(fs.readFileSync(path.join(assets, asset))).digest('hex');
  const record = checksum === 'missing' ? `${hash}  unrelated.tar.gz` : `${checksum === 'invalid' ? '0'.repeat(64) : hash}  ${asset}`;
  fs.writeFileSync(path.join(assets, `trivy_${version.slice(1)}_checksums.txt`), `${record}\n`);
  fs.writeFileSync(path.join(bin, 'uname'), '#!/bin/sh\nif [ "$1" = -s ]; then echo Linux; else echo "$MOCK_ARCH"; fi\n', { mode: 0o755 });
  fs.writeFileSync(path.join(bin, 'curl'), `#!${process.execPath}
const fs = require('node:fs');
const path = require('node:path');
const args = process.argv.slice(2);
const url = args.at(-1);
fs.appendFileSync(process.env.CURL_LOG, url + '\\n');
if (process.env.CURL_FAILURE) process.exit(Number(process.env.CURL_FAILURE));
if (!url.startsWith('https://github.com/aquasecurity/trivy/releases/download/' + process.env.TRIVY_VERSION + '/')) process.exit(22);
fs.copyFileSync(path.join(process.env.MOCK_ASSETS, path.basename(new URL(url).pathname)), args[args.indexOf('--output') + 1]);
`, { mode: 0o755 });
  const env = {
    ...process.env, PATH: `${bin}:${process.env.PATH}`, TRIVY_VERSION: version,
    RUNNER_TEMP: root, GITHUB_PATH: githubPath, MOCK_ARCH: arch,
    MOCK_ASSETS: assets, CURL_LOG: path.join(root, 'curl.log'), CURL_FAILURE: curlFailure,
  };
  return {
    root, env, githubPath,
    run: overrides => spawnSync('bash', [path.join(actionDir, 'install.sh')], {
      env: { ...env, ...overrides }, encoding: 'utf8', timeout: 10000,
    }),
  };
}

for (const arch of ['x86_64', 'aarch64']) {
  test(`verified GitHub release is installed and exported on ${arch}`, t => {
    const f = fixture(t, { arch });
    const result = f.run();
    assert.equal(result.status, 0, result.stderr);
    const installed = fs.readFileSync(f.githubPath, 'utf8').trim();
    assert.equal(execFileSync(path.join(installed, 'trivy'), ['--version'], { encoding: 'utf8' }).trim(), `Version: ${version.slice(1)}`);
    assert.equal(fs.readFileSync(f.env.CURL_LOG, 'utf8').trim().split('\n').length, 2);
    assert.ok(!fs.readdirSync(f.root).some(name => name.startsWith('trivy-download.')));
  });
}

for (const checksum of ['invalid', 'missing']) {
  test(`${checksum} checksum stops installation before exposing a binary`, t => {
    const f = fixture(t, { checksum });
    assert.notEqual(f.run().status, 0);
    assert.equal(fs.readFileSync(f.githubPath, 'utf8'), '');
    assert.equal(fs.existsSync(path.join(f.root, `trivy-${version}/bin/trivy`)), false);
  });
}

test('download failure preserves failure and does not export an install', t => {
  const f = fixture(t, { curlFailure: '6' });
  assert.equal(f.run().status, 6);
  assert.equal(fs.readFileSync(f.githubPath, 'utf8'), '');
  assert.ok(!fs.readdirSync(f.root).some(name => name.startsWith('trivy-download.')));
});

test('unversioned downloads and unsupported architectures stop before the network', t => {
  const f = fixture(t);
  for (const overrides of [{ TRIVY_VERSION: 'latest' }, { MOCK_ARCH: 'unknown' }]) {
    assert.equal(f.run(overrides).status, 1);
    assert.equal(fs.existsSync(f.env.CURL_LOG), false);
  }
});

test('each scan uses the verified installation and never invokes the upstream installer', () => {
  let scanCount = 0;
  let installationCount = 0;
  for (const name of ['container-images.yml', 'quality-gate.yml']) {
    const workflow = fs.readFileSync(path.join(repoRoot, '.github/workflows', name), 'utf8');
    for (const job of workflow.split(/\n  [\w-]+:\n/)) {
      const scans = [...job.matchAll(/uses: aquasecurity\/trivy-action@[^\n]+\n([\s\S]*?)(?=\n      - |$)/g)];
      if (!scans.length) continue;
      const installIndex = job.indexOf('uses: ./.github/actions/setup-trivy');
      assert.ok(installIndex >= 0 && installIndex < scans[0].index, `${name}: install before scanning`);
      assert.equal((job.match(/uses: \.\/.github\/actions\/setup-trivy/g) || []).length, 1);
      for (const scan of scans) assert.match(scan[1], /skip-setup-trivy: true/);
      scanCount += scans.length;
      installationCount++;
    }
  }
  assert.equal(scanCount, 8);
  assert.equal(installationCount, 3);
});
