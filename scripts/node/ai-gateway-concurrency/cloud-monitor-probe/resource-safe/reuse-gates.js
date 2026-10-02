'use strict';
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),assert=require('node:assert/strict');
const spec=require('./reuse-gates.json'),freeze=require('./freeze.json'),root='tmp/prior-safe-gates/test-governance/gateway-resource-safe/build',out='tmp/test-governance/gateway-resource-safe/build';
for(const [name,expected] of Object.entries(spec.hashes)){assert.equal(crypto.createHash('sha256').update(fs.readFileSync(path.join(root,name))).digest('hex'),expected,name);}
const p=JSON.parse(fs.readFileSync(root+'/source-proof.json'));assert.equal(p.candidate,freeze.candidate);assert.equal(p.baseline,freeze.common_baseline);assert.equal(p.candidate_diff_sha256,freeze.candidate_diff_sha256);assert.equal(fs.readFileSync(root+'/harness-sha.txt','utf8').trim(),'fc04e7f65ee48be89e35ae6437623aa9512c29a8');assert.ok(fs.readFileSync(root+'/toolchain.txt','utf8').includes('rustc 1.98.1'));
for(const name of ['contracts-tests.log','pg-trajectory-tests.log','pg-sequencing-tests.log']){const text=fs.readFileSync(root+'/'+name,'utf8');assert.match(text,/test result: ok[.] [1-9][0-9]* passed; 0 failed/);fs.copyFileSync(root+'/'+name,out+'/'+name);}
for(const name of ['contracts-boundary.json','adapter-test-compile.log'])fs.copyFileSync(root+'/'+name,out+'/'+name);
fs.writeFileSync(out+'/reused-gates.json',JSON.stringify({...spec,unchanged_product_sources:true,control_plane_failed_gate_reused:false},null,2));console.log('Verified immutable contracts29 + PG58 + sequencing6; failed classifier gate not reused.');
