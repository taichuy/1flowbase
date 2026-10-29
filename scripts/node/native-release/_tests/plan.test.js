const test = require('node:test');
const assert = require('node:assert/strict');
const { validateBuildInputs, selectContainerRelease } = require('../plan.js');

const base = { eventName: 'push', ref: 'refs/heads/latest', webEnabled: true, apiEnabled: true, webTag: 'v0.5.9', apiTag: 'v0.5.9', version: '0.5.9' };
test('a latest version release selects Docker and all native assets at one version', () => {
  assert.deepEqual(selectContainerRelease(base), { enabled: true, tag: 'v0.5.9' });
  assert.deepEqual(selectContainerRelease({ ...base, webEnabled: false }), { enabled: true, tag: 'v0.5.9' });
  assert.deepEqual(selectContainerRelease({ ...base, apiEnabled: false }), { enabled: true, tag: 'v0.5.9' });
});
test('ordinary source pushes and manual component rebuilds do not publish another release', () => {
  assert.equal(selectContainerRelease({ ...base, webEnabled: false, apiEnabled: false }).enabled, false);
  assert.equal(selectContainerRelease({ ...base, ref: 'refs/heads/dev' }).enabled, false);
  assert.equal(selectContainerRelease({ ...base, eventName: 'workflow_dispatch', apiTag: '' }).enabled, false);
});
test('official versions cannot mix manifests or silently accept a VERSION-only bump', () => {
  for (const change of [{ apiTag: 'v0.5.8' }, { webTag: 'v0.5.8' }, { version: '0.6.0' }, { version: '0.5.9-beta.1' }, { webEnabled: false, apiEnabled: false, version: '0.6.0' }]) {
    assert.throws(() => selectContainerRelease({ ...base, ...change }), /versions to match/);
  }
});
test('manual single-platform validation remains available but publication requires a tag and all platforms', () => {
  assert.doesNotThrow(() => validateBuildInputs({ target: 'windows-x64' }));
  assert.doesNotThrow(() => validateBuildInputs({ target: 'all', tag: 'v0.5.9-native.1', publish: true }));
  assert.throws(() => validateBuildInputs({ target: 'all', publish: true }), /release_tag/);
  assert.throws(() => validateBuildInputs({ target: 'windows-x64', tag: 'v0.5.9' }), /all targets/);
  assert.throws(() => validateBuildInputs({ target: 'all', tag: 'latest' }), /valid version/);
  assert.throws(() => validateBuildInputs({ target: 'unknown' }), /Unsupported/);
});
