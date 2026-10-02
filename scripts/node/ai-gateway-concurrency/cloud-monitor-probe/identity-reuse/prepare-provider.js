'use strict';
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),assert=require('node:assert/strict');
const freeze=require('./freeze.json');
const hash=p=>crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
const root='tmp/provider-artifact';
for(const [name,expected] of Object.entries(freeze.provider_packages_sha256)) {
 const source=path.join(root,'protocol-reuse/ai-gateway-quality-gate/packages',name);
 const files=fs.readdirSync(source).filter(n=>n.endsWith('.1flowbasepkg'));
 assert.equal(files.length,1);assert.equal(hash(path.join(source,files[0])),expected);
 const target=path.join('tmp/protocol-reuse/ai-gateway-quality-gate/packages',name);
 fs.mkdirSync(target,{recursive:true});fs.copyFileSync(path.join(source,files[0]),path.join(target,files[0]));
}
const file=path.join(root,'protocol-reuse/ai-gateway-concurrency/provider-provenance.json');
const p=JSON.parse(fs.readFileSync(file));assert.equal(p.official_source_sha,freeze.provider_source);
for(const [name,h]of Object.entries(freeze.provider_packages_sha256))assert.equal(p.providers[name].validated_package_sha256,h);
fs.mkdirSync('tmp/protocol-reuse/ai-gateway-concurrency',{recursive:true});
fs.copyFileSync(file,'tmp/protocol-reuse/ai-gateway-concurrency/provider-provenance.json');
console.log('Exact immutable Provider packages verified; no sealed old API binary used');
