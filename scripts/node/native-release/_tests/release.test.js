const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { ASSETS, resolveTagCommit, validateReleaseTarget, publishRelease, releaseNotes } = require('../release.js');
const sha = '1'.repeat(40);
const repo = 'taichuy/1flowbase';

test('release identity resolves tags and rejects mismatched source', () => {
  const tag = 'v0.5.8-native.1';
  assert.equal(resolveTagCommit([], tag, () => { throw new Error('unexpected lookup'); }), null);
  assert.equal(resolveTagCommit([{ ref: `refs/tags/${tag}`, object: { type: 'commit', sha } }], tag, () => null), sha);
  const refs = [{ ref: `refs/tags/${tag}`, object: { type: 'tag', sha: 'annotation' } }];
  assert.equal(resolveTagCommit(refs, tag, id => { assert.equal(id, 'annotation'); return { object: { type: 'commit', sha } }; }), sha);
  assert.throws(() => validateReleaseTarget('2'.repeat(40), null, sha), /another commit/);
  assert.doesNotThrow(() => validateReleaseTarget(sha, { isDraft: true, targetCommitish: sha }, sha));
  assert.throws(() => validateReleaseTarget(null, { isDraft: false, targetCommitish: sha }, sha), /exact candidate/);
  assert.throws(() => validateReleaseTarget(sha, { isDraft: true, targetCommitish: 'latest' }, sha), /exact candidate/);
});
function fixture(t, { tag = 'v0.5.9', commit = null, release = null, uploadFails = false, moveTagOnUpload = false } = {}) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'native-publish-'));
  t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  for (const name of ASSETS) fs.writeFileSync(path.join(directory, name), 'validated-archive-fixture');
  const calls = [];
  const run = (command, args) => {
    assert.equal(command, 'gh'); calls.push(args);
    const ok = value => ({ status: 0, stdout: JSON.stringify(value), stderr: '' });
    if (args[0] === 'release' && args[1] === 'view') {
      assert.deepEqual(args.slice(2), [tag, '--json', 'isDraft,targetCommitish,isPrerelease,assets']);
      return release ? ok({ isDraft: release.draft, targetCommitish: release.target_commitish, isPrerelease: release.prerelease, assets: release.assets }) : { status: 1, stderr: 'release not found' };
    }
    if (args[0] === 'api' && args[1].includes('/git/matching-refs/')) return ok(commit ? [{ ref: `refs/tags/${tag}`, object: { type: 'commit', sha: commit } }] : []);
    if (args[0] === 'api' && args.includes('POST')) { commit = args.find(arg => arg.startsWith('sha=')).slice(4); return ok({}); }
    if (args[0] === 'release' && args[1] === 'create') { assert.ok(args.includes('--draft')); return ok({}); }
    if (args[0] === 'release' && args[1] === 'upload') {
      assert.deepEqual(args.slice(3, -1).map(file => path.basename(file)), ASSETS);
      if (moveTagOnUpload) commit = null;
      return uploadFails ? { status: 1, stderr: 'upload failed' } : ok({});
    }
    if (args[0] === 'release' && args[1] === 'edit') return ok({});
    throw new Error(`Unexpected GitHub operation: ${args}`);
  };
  return { directory, calls, run, options: { tag, sha, repo, directory, kind: 'combined', publish: true, runId: '123' } };
}
const mutations = calls => calls.filter(args => (args[0] === 'release' && ['create', 'upload', 'edit'].includes(args[1])) || args.includes('POST'));

test('combined release stays draft until all five archives are uploaded, then publishes stable', t => {
  const f = fixture(t);
  publishRelease(f.options, f.run);
  const writes = mutations(f.calls);
  assert.deepEqual(writes.map(args => args[0] === 'api' ? 'tag' : args[1]), ['tag', 'create', 'upload', 'edit']);
  assert.ok(writes[1].includes('--generate-notes'));
  assert.ok(!writes[1].includes('--prerelease'));
  assert.ok(writes[3].includes('--prerelease=false'));
  const notes = fs.readFileSync(path.join(f.directory, '.release-notes.md'), 'utf8');
  for (const text of ['## Docker Images', '1flowbase-web:v0.5.9', '1flowbase-api-server:v0.5.9', '## Native portable distributions', 'Target commit', sha, 'actions/runs/123']) assert.ok(notes.includes(text), text);
});
test('standalone native publication remains a prerelease and can leave a draft', t => {
  const f = fixture(t, { tag: 'v0.5.9-native.1' });
  publishRelease({ ...f.options, kind: 'native', publish: false }, f.run);
  assert.ok(f.calls.find(args => args[1] === 'create').includes('--prerelease'));
  assert.ok(!f.calls.some(args => args[1] === 'edit'));
  assert.ok(!fs.readFileSync(path.join(f.directory, '.release-notes.md'), 'utf8').includes('## Docker Images'));
});
test('missing or wrong platform assets fail before any GitHub mutation', t => {
  const f = fixture(t);
  fs.unlinkSync(path.join(f.directory, ASSETS[0]));
  fs.writeFileSync(path.join(f.directory, '1flowbase-windows-arm64.zip'), 'wrong platform');
  assert.throws(() => publishRelease(f.options, f.run), /All five/);
  assert.equal(f.calls.length, 0);
});
test('upload failure or a tag race never publishes the draft', t => {
  for (const failure of [{ uploadFails: true }, { moveTagOnUpload: true }]) {
    const f = fixture(t, failure);
    assert.throws(() => publishRelease(f.options, f.run), /upload failed|tag changed/);
    assert.ok(!f.calls.some(args => args[1] === 'edit'));
  }
});
test('mismatched tags and draft candidates cannot be reused', t => {
  for (const options of [{ commit: '2'.repeat(40) }, { commit: sha, release: { draft: true, target_commitish: 'latest', prerelease: false } }]) {
    const f = fixture(t, options);
    assert.throws(() => publishRelease(f.options, f.run), /another commit|exact candidate/);
    assert.equal(mutations(f.calls).length, 0);
  }
});
test('a complete published release for this SHA is a no-op; public assets are never overwritten', t => {
  const release = { draft: false, target_commitish: sha, prerelease: false, assets: ASSETS.map(name => ({ name, state: 'uploaded', size: 1 })) };
  const f = fixture(t, { commit: sha, release });
  assert.equal(publishRelease(f.options, f.run).existing, true);
  assert.equal(mutations(f.calls).length, 0);
  release.assets.pop();
  assert.throws(() => publishRelease(f.options, f.run), /missing native assets/);
  assert.equal(mutations(f.calls).length, 0);
});
test('a partial draft at the same candidate resumes without creating a second release', t => {
  const f = fixture(t, { commit: sha, release: { draft: true, target_commitish: sha, prerelease: false } });
  publishRelease(f.options, f.run);
  assert.deepEqual(mutations(f.calls).map(args => args[1]), ['upload', 'edit']);
});
test('Docker installation instructions and native instructions coexist in generated notes', () => {
  const notes = releaseNotes({ kind: 'combined', tag: 'v0.5.9', sha, repo, runId: '123' });
  for (const text of ['Linux/macOS:', 'Windows PowerShell:', 'Windows CMD:', 'scripts/shell/docker-deploy.sh', 'scripts/powershell/docker-deploy.ps1', 'NoProfile -ExecutionPolicy Bypass', 'upgrade.cmd / upgrade.sh', '不创建备份']) assert.ok(notes.includes(text), text);
});
