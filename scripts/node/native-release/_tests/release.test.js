const test = require('node:test');
const assert = require('node:assert/strict');
const { resolveTagCommit, validateReleaseTarget } = require('../release.js');

test('release identity resolves lightweight and annotated tags and rejects mismatched source', () => {
  const tag = 'v0.5.8-native.1';
  const sha = '1'.repeat(40);
  assert.equal(resolveTagCommit([],tag,()=>{throw new Error('unexpected lookup');}),null);
  assert.equal(resolveTagCommit([{ref:`refs/tags/${tag}`,object:{type:'commit',sha}}],tag,()=>null),sha);
  const refs=[{ref:`refs/tags/${tag}`,object:{type:'tag',sha:'annotation'}}];
  assert.equal(resolveTagCommit(refs,tag,id=>{assert.equal(id,'annotation');return {object:{type:'commit',sha}};}),sha);
  assert.throws(()=>validateReleaseTarget('2'.repeat(40),null,sha),/another commit/);
  assert.doesNotThrow(()=>validateReleaseTarget(sha,{isDraft:true,targetCommitish:sha},sha));
  assert.throws(()=>validateReleaseTarget(sha,{isDraft:false,targetCommitish:sha},sha),/draft/);
  assert.throws(()=>validateReleaseTarget(sha,{isDraft:true,targetCommitish:'dev'},sha),/exact candidate/);
});
