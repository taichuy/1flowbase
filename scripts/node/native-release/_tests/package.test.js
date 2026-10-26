const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { packageName, stagePackage } = require('../package.js');

test('artifact names contain only product, OS and architecture', () => {
  assert.equal(packageName('linux', 'x64'), '1flowbase-linux-x64');
  assert.equal(packageName('macos', 'arm64'), '1flowbase-macos-arm64');
  assert.equal(packageName('windows', 'x64'), '1flowbase-windows-x64');
  assert.throws(() => packageName('../linux', 'x64'));
  assert.throws(() => packageName('linux', 'x64-deadbeef'));
});

test('stages runtime inputs without copying developer secrets or replacing existing packages', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'native-package-'));
  try {
    const write = (name, content = 'fixture') => { const file = path.join(root, name); fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, content); };
    for (const name of ['api-bin', 'recovery-bin', 'dist/index.html', 'LICENSE', 'docker/native/README.md', 'docker/native/README_CN.md', 'api/plugins/host-extensions/host.yaml', 'api/plugins/sets/default.yaml', 'api/plugins/capability-plugins/index.js', 'api/plugins/installed/secret', 'api/.env', 'resources/application-templates/receipt.json']) write(name);
    for (const tool of ['postgres', 'pg_ctl', 'initdb', 'psql', 'createdb', 'pg_dump', 'pg_restore']) write(`pg/bin/${tool}`);
    write('web/app/package.json', '{"version":"0.1.0"}');
    const options = { repoRoot: root, apiBinary: path.join(root, 'api-bin'), recoveryBinary: path.join(root, 'recovery-bin'), webDir: path.join(root, 'dist'), postgresDir: path.join(root, 'pg'), outDir: path.join(root, 'out'), platform: 'linux', arch: 'x64', resourcesDir: path.join(root, 'resources') };
    const stage = stagePackage(options);
    assert.ok(fs.existsSync(path.join(stage, 'postgres/bin/pg_restore')));
    assert.ok(fs.existsSync(path.join(stage, 'start.sh')));
    assert.ok(fs.existsSync(path.join(stage, 'api/resources/application-templates/receipt.json')));
    assert.ok(!fs.existsSync(path.join(stage, 'api/.env')));
    assert.ok(!fs.existsSync(path.join(stage, 'api/plugins/installed')));
    const manifest = JSON.parse(fs.readFileSync(path.join(stage, 'native-distribution.json')));
    assert.equal(manifest.http_port, 7800);
    assert.equal(manifest.postgres_port, 35432);
    assert.throws(() => stagePackage(options), /EEXIST/);
    fs.rmSync(path.join(root, 'pg/bin/pg_restore'));
    assert.throws(() => stagePackage({ ...options, outDir: path.join(root, 'out2') }), /pg_restore/);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});
