'use strict';
const test=require('node:test'),assert=require('node:assert/strict');
const{prove}=require('../prepare-packages'),f=require('../freeze.json');
const valid=()=>({metadata:{id:f.package_artifact_id,expired:false,digest:'sha256:'+f.package_artifact_sha256,workflow_run:{id:f.package_artifact_run,head_sha:f.package_artifact_controls_sha}},provider:{official_source_sha:f.provider_source,providers:Object.fromEntries(Object.entries(f.provider_packages_sha256).map(([name,sha])=>[name,{validated_package_sha256:sha}]))},packages:{...f.provider_packages_sha256}});
test('immutable package provenance accepted without old API binary reuse',()=>{const p=prove(valid());assert.equal(p.status,'pass');assert.equal(p.binary_reused,false);});
test('foreign expired archive or package bytes fail closed',()=>{for(const mutate of [v=>v.metadata.expired=true,v=>v.metadata.workflow_run.head_sha='0'.repeat(40),v=>v.metadata.digest='sha256:'+'0'.repeat(64),v=>v.provider.official_source_sha='0'.repeat(40),v=>v.packages.openai='0'.repeat(64)]){const v=valid();mutate(v);assert.throws(()=>prove(v));}});
