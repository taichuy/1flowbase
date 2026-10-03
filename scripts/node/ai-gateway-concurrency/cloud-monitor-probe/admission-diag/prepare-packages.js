'use strict';
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),assert=require('node:assert/strict');
const freeze=require('./freeze.json');
const hash=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
function prove({metadata,provider,packages}){
 assert.equal(metadata.id,freeze.package_artifact_id);assert.equal(metadata.expired,false);assert.equal(metadata.digest,'sha256:'+freeze.package_artifact_sha256);assert.equal(metadata.workflow_run.id,freeze.package_artifact_run);assert.equal(metadata.workflow_run.head_sha,freeze.package_artifact_controls_sha);
 assert.equal(provider.official_source_sha,freeze.provider_source);for(const[name,want]of Object.entries(freeze.provider_packages_sha256)){assert.equal(packages[name],want);assert.equal(provider.providers[name].validated_package_sha256,want);}
 return{status:'pass',provider_source:freeze.provider_source,archive_id:freeze.package_artifact_id,archive_run:freeze.package_artifact_run,archive_digest:metadata.digest,packages,binary_reused:false,scope:'only official immutable package bytes reused; previous API binaries not read/executed'};
}
function main(){
 const root='tmp/admission-package-reuse',out='tmp/test-governance/gateway-admission-diag/build';fs.mkdirSync(out,{recursive:true});const read=p=>JSON.parse(fs.readFileSync(p));const provider=read(path.join(root,'protocol-reuse/ai-gateway-concurrency/provider-provenance.json')),packages={};
 for(const[name,want]of Object.entries(freeze.provider_packages_sha256)){const dir=path.join(root,'protocol-reuse/ai-gateway-quality-gate/packages',name),files=fs.readdirSync(dir).filter(p=>p.endsWith('.1flowbasepkg'));assert.equal(files.length,1);packages[name]=hash(path.join(dir,files[0]));assert.equal(packages[name],want);const dest=path.join('tmp/protocol-reuse/ai-gateway-quality-gate/packages',name);fs.mkdirSync(dest,{recursive:true});fs.copyFileSync(path.join(dir,files[0]),path.join(dest,files[0]));}
 const receipt=prove({metadata:read(path.join(out,'package-artifact-metadata.json')),provider,packages});const provenance='tmp/protocol-reuse/ai-gateway-concurrency';fs.mkdirSync(provenance,{recursive:true});fs.writeFileSync(path.join(provenance,'provider-provenance.json'),JSON.stringify(provider,null,2));fs.writeFileSync(path.join(out,'package-reuse-proof.json'),JSON.stringify(receipt,null,2));fs.copyFileSync(path.join(__dirname,'freeze.json'),path.join(out,'freeze.json'));fs.writeFileSync(path.join(out,'harness-sha.txt'),process.env.WORKFLOW_SHA+'\n');console.log('Exact official provider packages reused; old API binaries forbidden; single fresh diagnostic build required');
}
if(require.main===module)main();module.exports={prove};
