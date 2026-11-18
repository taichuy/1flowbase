'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const { createHash } = require('node:crypto');
const { execFile } = require('node:child_process');
const { once } = require('node:events');
const { promisify } = require('node:util');

async function verifyArchive(stage, archive, evidence) {
  stage = path.resolve(stage);
  archive = path.resolve(archive);
  evidence = path.resolve(evidence);
  assert.ok(!fs.existsSync(evidence), 'Use a fresh updater evidence directory');
  fs.mkdirSync(evidence, { recursive: true, mode: 0o700 });
  const baseline = path.join(evidence, 'baseline');
  const scratch = path.join(evidence, 'scratch');
  fs.mkdirSync(baseline);
  fs.mkdirSync(scratch, { mode: 0o700 });
  const manifestName = 'native-distribution.json';
  const candidate = JSON.parse(fs.readFileSync(path.join(stage, manifestName), 'utf8'));
  const baselineManifest = JSON.stringify({ ...candidate, version: '0.0.0-native.0', build_id: candidate.build_id + '-download-baseline-fixture' });
  fs.writeFileSync(path.join(baseline, manifestName), baselineManifest);
  const name = path.basename(archive);
  const size = fs.statSync(archive).size;
  const hash = createHash('sha256');
  for await (const chunk of fs.createReadStream(archive)) hash.update(chunk);
  const digest = hash.digest('hex');
  const badDigest = (digest[0] === '0' ? '1' : '0') + digest.slice(1);
  const executable = path.join(stage, process.platform === 'win32' ? '1flowbase.exe' : '1flowbase');
  let mode;
  let requests;
  const server = http.createServer((req, res) => {
    requests.push(`${req.method} ${req.url}`);
    if (req.method === 'GET' && req.url === '/releases') {
      if (mode === 'redirect') {
        res.writeHead(302, { location: '/redirected' }).end();
        return;
      }
      res.setHeader('content-type', 'application/json');
      res.end(JSON.stringify([{
        tag_name: `v${candidate.version}`, draft: false, prerelease: candidate.version.split('+')[0].includes('-'),
        assets: [{ name, size, browser_download_url: `${origin}/archive`, digest: `sha256:${mode === 'bad-digest' ? badDigest : digest}` }],
      }]));
    } else if (req.method === 'GET' && req.url === '/archive') {
      res.setHeader('content-length', size);
      const body = fs.createReadStream(archive);
      body.on('error', error => res.destroy(error));
      res.on('close', () => body.destroy());
      body.pipe(res);
    } else {
      res.writeHead(404).end();
    }
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const origin = `http://127.0.0.1:${server.address().port}`;
  async function run(probe, expectedError) {
    mode = probe;
    requests = [];
    let result;
    let error;
    try {
      result = await promisify(execFile)(executable, ['--verify-update-archive', baseline, `${origin}/releases`], {
        cwd: evidence, env: { ...process.env, TMPDIR: scratch, TMP: scratch, TEMP: scratch }, timeout: 200000,
      });
    } catch (failure) {
      error = failure;
      result = failure;
    }
    fs.writeFileSync(path.join(evidence, `command-${probe}.log`), `${result.stdout || ''}${result.stderr || ''}`);
    if (expectedError) {
      assert.ok(Number.isInteger(error?.code) && error.code !== 0, `${probe}: expected updater refusal`);
      assert.match(error.stderr, expectedError);
    } else if (error) {
      throw new Error(`Updater failed; inspect command-${probe}.log`, { cause: error });
    }
    assert.deepEqual(requests, probe === 'redirect' ? ['GET /releases'] : ['GET /releases', 'GET /archive']);
    assert.deepEqual(fs.readdirSync(baseline), [manifestName], 'Updater must not write into the baseline');
    assert.equal(fs.readFileSync(path.join(baseline, manifestName), 'utf8'), baselineManifest);
    return result.stdout;
  }
  try {
    await run('redirect', /fixtures must not redirect/);
    await run('bad-digest', /Update download checksum\/size mismatch/);
    assert.deepEqual(fs.readdirSync(scratch), [], 'Rejected archives must leave no candidate');
    const stdout = await run('valid');
    const receipt = /^Verified update package: (.+)$/m.exec(stdout);
    assert.ok(receipt, 'Updater did not produce a verified package');
    const packageRoot = fs.realpathSync(receipt[1]);
    const relative = path.relative(fs.realpathSync(scratch), packageRoot);
    assert.match(relative, /^1flowbase-verify-[a-f0-9-]+[\\/]download[\\/]package$/);
    const actual = JSON.parse(fs.readFileSync(path.join(packageRoot, manifestName), 'utf8'));
    assert.equal(actual.version, candidate.version);
    assert.equal(actual.build_id, candidate.build_id);
    assert.ok(fs.lstatSync(path.join(packageRoot, '.env')).isFile());
    assert.ok(!fs.existsSync(path.join(packageRoot, 'data')));
    fs.writeFileSync(`${archive}.sha256`, `${digest}  ${name}\n`);
    const result = { package_root: packageRoot, archive: name, sha256: digest, size, version: actual.version, passed: ['loopback-no-redirects', 'bad-digest-refused', 'download-and-extract', 'candidate-manifest-and-layout', 'baseline-unchanged'] };
    fs.writeFileSync(path.join(evidence, 'result.json'), JSON.stringify(result, null, 2) + '\n');
    console.log(JSON.stringify(result));
  } finally {
    const closed = once(server, 'close');
    server.close();
    server.closeAllConnections();
    await closed;
  }
}

if (require.main === module) verifyArchive(process.argv[2], process.argv[3], process.argv[4]).catch(error => { console.error(error); process.exitCode = 1; });
module.exports = { verifyArchive };
