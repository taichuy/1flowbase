'use strict';
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const hashes={baseline:'de3a8498f854077db439eed6ddddd05897a77f5cdbb524122fe08b997391f889',candidate:'14cbf8eb85b464b0cccbe7463dc8c451296de22b4e7ef9e7df645b0372fe2ba1'};
function reuseVerifiedGates(root){
 const manifest=JSON.parse(fs.readFileSync(path.join(root,'manifest.json')));assert.equal(manifest.workflow,'ea8013433c6031bf8c72a59f4a6253b33e967473');const rows={};
 for(const arm of ['baseline','candidate']){
  const bytes=fs.readFileSync(path.join(root,arm,'result.json'));assert.equal(crypto.createHash('sha256').update(bytes).digest('hex'),hashes[arm]);const f=JSON.parse(bytes);assert.equal(f.source,manifest[arm]);
  assert.equal(f.lifecycle.verdict,'PASS');assert.deepEqual(f.lifecycle.rows.map(x=>x.id),['client-disconnect','native-cancel','upstream-interruption']);assert.equal(f.retry.verdict,'PASS');assert.equal(f.retry.rows[0].id,'retry/responses-websocket');assert.equal(f.retry.rows[0].attempts.length,2);
  // The former overall failure was the fixture's incomplete final-text oracle;
  // reuse only unchanged passing groups, not the failed tool recovery.
  assert.ok(f.error.includes('Exact expected text'));assert.ok(f.error.includes('chunk-1 marker-1chunk-2 marker-2'));rows[arm]={run:36975125248,result_sha256:hashes[arm],lifecycle:f.lifecycle,retry:f.retry};
 }
 return rows;
}
module.exports={reuseVerifiedGates};
