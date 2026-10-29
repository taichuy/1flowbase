const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { packageName, stagePackage, archivePackage } = require('../package.js');
const { unpack } = require('../unpack.js');

test('artifact names contain only product, OS and architecture', () => {
  assert.equal(packageName('linux', 'x64'), '1flowbase-linux-x64');
  assert.equal(packageName('macos', 'arm64'), '1flowbase-macos-arm64');
  assert.equal(packageName('windows', 'x64'), '1flowbase-windows-x64');
  assert.throws(() => packageName('../linux', 'x64'));
  assert.throws(() => packageName('linux', 'x64-deadbeef'));
});

test('stages runtime inputs without copying developer secrets or replacing existing packages', () => {
  const platform = process.platform === 'win32' ? 'windows' : process.platform === 'darwin' ? 'macos' : 'linux';
  const suffix = platform === 'windows' ? '.exe' : '';
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'native-package-'));
  try {
    const write = (name, content = 'fixture') => { const file = path.join(root, name); fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, content); };
    for (const name of ['api-bin', 'recovery-bin', 'dist/index.html', 'LICENSE', 'docker/native/README.md', 'docker/native/README_CN.md', 'api/plugins/host-extensions/host.yaml', 'api/plugins/sets/default.yaml', 'api/plugins/capability-plugins/index.js', 'api/plugins/installed/secret', 'api/.env', 'resources/application-templates/receipt.json']) write(name);
    for (const tool of ['postgres', 'pg_ctl', 'initdb', 'psql', 'createdb', 'pg_dump', 'pg_restore']) write(`pg/bin/${tool}${suffix}`);
    write('api/apps/api-server/.env.example', 'API_ENV=development\nAPI_SERVER_ADDR=0.0.0.0:7800\nAPI_AUTH_PUBLIC_ORIGIN=http://127.0.0.1:3100\nVITE_DEV_SERVER_PORT=3100\nBOOTSTRAP_ROOT_ACCOUNT=root\nBOOTSTRAP_ROOT_EMAIL=root@example.com\nBOOTSTRAP_ROOT_PASSWORD=example-default\nBOOTSTRAP_WORKSPACE_NAME=example-workspace\n');
    write('web/app/package.json', '{"version":"0.1.0"}');
    const options = { repoRoot: root, apiBinary: path.join(root, 'api-bin'), recoveryBinary: path.join(root, 'recovery-bin'), webDir: path.join(root, 'dist'), postgresDir: path.join(root, 'pg'), outDir: path.join(root, 'out'), platform, arch: 'x64', resourcesDir: path.join(root, 'resources') };
    const stage = stagePackage(options);
    assert.ok(fs.existsSync(path.join(stage, 'postgres/bin/pg_restore' + suffix)));
    assert.ok(fs.existsSync(path.join(stage, platform === 'windows' ? '1flowbase.exe' : 'start.sh')));
    assert.ok(fs.existsSync(path.join(stage, 'api/resources/application-templates/receipt.json')));
    assert.ok(!fs.existsSync(path.join(stage, 'api/.env')));
    assert.ok(!fs.existsSync(path.join(stage, 'api/plugins/installed')));
    const manifest = JSON.parse(fs.readFileSync(path.join(stage, 'native-distribution.json')));
    assert.equal(manifest.format_version, 1);
    assert.ok(manifest.files['.env.example']);
    assert.ok(!manifest.files['.env']);
    assert.ok(!manifest.files['data/settings.json']);
    const env = fs.readFileSync(path.join(stage, '.env'), 'utf8');
    assert.match(env, /^API_ENV=production$/m);
    assert.match(env, /^BOOTSTRAP_ROOT_PASSWORD=example-default$/m);
    assert.match(env, /^API_SERVER_ADDR=127\.0\.0\.1:7800$/m);
    assert.match(env, /^API_AUTH_PUBLIC_ORIGIN=http:\/\/127\.0\.0\.1:7800$/m);
    assert.ok(!env.includes('VITE_DEV_SERVER_PORT'));
    assert.equal(env, fs.readFileSync(path.join(stage, '.env.example'), 'utf8'));
    assert.ok(fs.existsSync(path.join(stage, platform === 'windows' ? 'upgrade.cmd' : 'upgrade.sh')));
    archivePackage(stage, platform);
    const extracted = unpack(stage, path.join(root, 'extracted'));
    assert.equal(fs.readFileSync(path.join(extracted, '.env'), 'utf8'), env);
    assert.equal(manifest.http_port, 7800);
    assert.equal(manifest.postgres_port, 35432);
    assert.throws(() => stagePackage(options), /EEXIST/);
    fs.rmSync(path.join(root, 'pg/bin/pg_restore' + suffix));
    assert.throws(() => stagePackage({ ...options, outDir: path.join(root, 'out2') }), /pg_restore/);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});
