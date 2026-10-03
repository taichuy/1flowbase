const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const repoRoot = path.resolve(__dirname, '../../../..');
const packager = path.join(repoRoot, 'scripts/shell/package-application-templates.sh');
function run(cmd, args, cwd) {
  return spawnSync(cmd, args, { cwd, encoding: 'utf8', maxBuffer: 1024 * 1024 });
}
function fixture(t, releaseVersion) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'application-template-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const source = path.join(root, 'source');
  const template = path.join(source, 'applications-demo/@test/demo');
  fs.mkdirSync(template, { recursive: true });
  const artifact = {
    schema_version: '1flowbase.portable-template/v1',
    release: { template_id: '@test/demo', release_version: releaseVersion, name: 'Demo' },
    pages: [], applications: [], data_models: [], plugins: [],
  };
  const bytes = `${JSON.stringify(artifact, null, 2)}\n`;
  fs.writeFileSync(path.join(template, 'template.json'), bytes);
  assert.equal(run('git', ['init', '-q'], source).status, 0);
  assert.equal(run('git', ['add', '.'], source).status, 0);
  assert.equal(run('git', ['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
    'commit', '-qm', 'Template fixture'], source).status, 0);
  const sha = run('git', ['rev-parse', 'HEAD'], source).stdout.trim();
  return { root, source, sha, bytes, output: path.join(root, 'output') };
}

test('image packager copies the immutable committed artifact, preserving bytes', (t) => {
  const f = fixture(t, 1);
  // Uncommitted source changes cannot silently alter an image release.
  fs.writeFileSync(path.join(f.source, 'applications-demo/@test/demo/template.json'), '{}');
  const result = run('sh', [packager, f.source, f.sha, f.output], repoRoot);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(fs.readFileSync(path.join(f.output, '@test/demo/template.json'), 'utf8'), f.bytes);
  const receipt = JSON.parse(fs.readFileSync(path.join(f.output, 'receipt.json'), 'utf8'));
  assert.equal(receipt.resolved_commit, f.sha);
  assert.equal(receipt.source_file_count, 1);
});

test('image packager rejects an invalid release and mutable references', (t) => {
  const f = fixture(t, 0);
  const invalid = run('sh', [packager, f.source, f.sha, f.output], repoRoot);
  assert.notEqual(invalid.status, 0);
  assert.match(invalid.stderr, /invalid application template/);
  assert.equal(fs.existsSync(path.join(f.output, 'receipt.json')), false);
  const mutable = run('sh', [packager, f.source, 'main', f.output], repoRoot);
  assert.notEqual(mutable.status, 0);
});

test('both API image targets inherit templates and compose passes the opt-out switch', () => {
  const dockerfile = fs.readFileSync(path.join(repoRoot, 'docker/api-server.Dockerfile'), 'utf8');
  assert.match(dockerfile, /COPY --from=application-template-bootstrap \/application-templates \/app\/api\/resources\/application-templates/);
  assert.match(dockerfile, /API_APPLICATION_TEMPLATE_ROOT=\/app\/api\/resources\/application-templates/);
  assert.match(dockerfile, /FROM runtime-base AS runtime\b/);
  assert.match(dockerfile, /FROM runtime-base AS runtime-prebuilt\b/);
  for (const file of ['docker-compose.yaml', 'docker-compose.dev.yaml', 'docker-compose.external-db.yaml']) {
    const source = fs.readFileSync(path.join(repoRoot, 'docker', file), 'utf8');
    assert.match(source, /API_APPLICATION_TEMPLATE_AUTO_UPDATE: \$\{API_APPLICATION_TEMPLATE_AUTO_UPDATE:-true\}/);
  }
});
