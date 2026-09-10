const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const root = path.resolve(__dirname, '../../../..');

test('AC-PRICING-1 default Docker price snapshot is immutable', () => {
  const source = fs.readFileSync(path.join(root, 'docker/api-server.Dockerfile'), 'utf8');
  assert.match(source, /^ARG MODEL_PRICING_REF=[a-f0-9]{40}$/m);
});

test('AC-PRICING-2 packaging accepts v2 and rejects v1 before producing a receipt', (t) => {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'flowbase-pricing-package-'));
  t.after(() => fs.rmSync(temp, { recursive: true, force: true }));
  const checkout = path.join(temp, 'official');
  const model = path.join(checkout, 'model-pricing/@zero/any');
  fs.mkdirSync(model, { recursive: true });
  const env = {
    ...process.env,
    GIT_CONFIG_COUNT: '1',
    GIT_CONFIG_KEY_0: `url.file://${checkout}.insteadOf`,
    GIT_CONFIG_VALUE_0: 'https://github.com/test/official.git',
  };
  function git(args) {
    const result = spawnSync('git', ['-C', checkout, ...args], { encoding: 'utf8', env });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  }
  git(['init', '--quiet']);
  const source = JSON.parse(fs.readFileSync(path.join(root,
    'api/apps/api-server/resources/model-pricing/@zero/any/pricing.json'), 'utf8'));
  for (const version of ['v2', 'v1']) {
    source.schema_version = `1flowbase.model-pricing-source/${version}`;
    fs.writeFileSync(path.join(model, 'pricing.json'), JSON.stringify(source));
    fs.writeFileSync(path.join(checkout, 'model-pricing/catalog-source.json'),
      JSON.stringify({ schema_version: source.schema_version, catalog_version: 'fixture' }));
    git(['add', '.']);
    git(['-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.com', 'commit', '--quiet', '-m', version]);
    const commit = git(['rev-parse', 'HEAD']);
    const output = path.join(temp, version);
    const result = spawnSync('sh', [path.join(root, 'scripts/shell/package-model-pricing-bootstrap.sh'),
      'test/official', commit, output], { env, encoding: 'utf8' });
    if (version === 'v2') {
      assert.equal(result.status, 0, result.stderr);
      const receipt = JSON.parse(fs.readFileSync(path.join(output, 'receipt.json'), 'utf8'));
      assert.equal(receipt.resolved_commit, commit);
      assert.equal(receipt.source_file_count, 1);
    } else {
      assert.notEqual(result.status, 0, 'old pricing sources must fail during packaging');
      assert.match(result.stderr, /@zero\/any\/pricing.json/);
      assert.equal(fs.existsSync(path.join(output, 'receipt.json')), false);
    }
  }
});
