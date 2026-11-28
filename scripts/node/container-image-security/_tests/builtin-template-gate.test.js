const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const root = path.resolve(__dirname, '../../../..');
const script = '.github/scripts/verify-builtin-application-templates.sh';
const dockerFixture = `#!${process.execPath}
const args = process.argv.slice(2);
const mode = process.env.FIXTURE_MODE;
if (args.includes('cat')) {
  const packages = mode === 'missing' ? [] : [{template_id:'@taichuy/gateway-demo',release_version:14}];
  if (mode === 'partial') packages.push({template_id:'@taichuy/second',release_version:1});
  console.log(JSON.stringify({schema_version:'1flowbase.application-template-bootstrap-receipt/v1',source_file_count:packages.length,packages}));
}
if (args[0] === 'run' && args.includes('candidate')) {
  if (args.some(arg => arg.includes('API_APPLICATION_TEMPLATE_AUTO_UPDATE') || arg.includes('API_APPLICATION_TEMPLATE_ROOT'))) process.exit(90);
}
if (args.includes('psql')) {
  const sql = args.at(-1);
  if (sql.includes('FROM application_template_releases')) {
    if (mode !== 'failed') console.log('@taichuy/gateway-demo\\t' + (mode === 'stale' ? '13' : '14'));
  } else if (sql.includes('frontstage_pages')) console.log(mode === 'no-page' ? 'f' : 't');
  else if (sql.includes('model_provider_instances')) console.log(mode === 'provider' ? '1' : '0');
  else process.exit(91);
}
`;

for (const [mode, failure] of Object.entries({
  success: null,
  missing: /missing required bundled template/,
  failed: /did not initialize every bundled template/,
  stale: /did not initialize every bundled template/,
  partial: /did not initialize every bundled template/,
  'no-page': /Gateway Demo route was not initialized/,
  provider: /unexpected model provider instances/,
})) {
  test(`built-in application template bootstrap gate: ${mode}`, t => {
    const bin = fs.mkdtempSync(path.join(os.tmpdir(), 'template-gate-test-'));
    t.after(() => fs.rmSync(bin, { recursive: true, force: true }));
    fs.writeFileSync(path.join(bin, 'docker'), dockerFixture, { mode: 0o755 });
    const result = spawnSync('bash', [path.join(root, script), 'candidate'], {
      env: { ...process.env, PATH: `${bin}:${process.env.PATH}`, FIXTURE_MODE: mode }, encoding: 'utf8', timeout: 12000,
    });
    if (failure) {
      assert.equal(result.status, 1, result.stderr);
      assert.match(result.stderr, failure);
    } else {
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /@taichuy\/gateway-demo\s+14/);
    }
  });
}

test('both image lanes require the same bootstrap gate, CD before promoting the tested digest', () => {
  const workflow = name => fs.readFileSync(path.join(root, '.github/workflows', name), 'utf8');
  const cd = workflow('container-images.yml');
  const gate = cd.indexOf(`bash ${script}`);
  const promote = cd.indexOf('Promote scanned api-server image to official tags');
  assert.ok(gate > 0 && gate < promote);
  assert.match(cd.slice(gate - 600, gate), /scan_image_ref \}\}@\$\{\{ steps\.api_server_candidate\.outputs\.digest/);
  assert.match(cd.slice(promote, promote + 850), /scan_image_ref \}\}@\$\{\{ steps\.api_server_candidate\.outputs\.digest/);
  const promotion = cd.slice(promote).split('\n\n', 1)[0];
  assert.match(promotion, /docker buildx imagetools create[^\n]*(?:\n[^\n]*--tag[^\n]*)+\n\s*"\$CANDIDATE_IMAGE"\s*$/u);
  const quality = workflow('quality-gate.yml');
  const imageJob = quality.slice(quality.indexOf('  container-images-gate:'), quality.indexOf('  ai-gateway-protocol-conformance:'));
  assert.match(imageJob, /bash \.github\/scripts\/verify-builtin-application-templates\.sh "1flowbase-api-server:\$QUALITY_GATE_TARGET_SHA"/);
});

test('application template packaging uses the builder platform for both target architectures', () => {
  const dockerfile = fs.readFileSync(path.join(root, 'docker/api-server.Dockerfile'), 'utf8');
  assert.match(dockerfile, /^FROM --platform=\$BUILDPLATFORM \S+ AS application-template-bootstrap$/m);
  assert.match(dockerfile, /^COPY --from=application-template-bootstrap \/application-templates \/app\/api\/resources\/application-templates$/m);
});
