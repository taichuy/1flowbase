'use strict';
const fs=require('node:fs'),cp=require('node:child_process'),path=require('node:path');
const {parseCargoTestCounts,requireExecutedCargoTests}=require('../../scripts/node/verify/cargo-test-results.js');
const source='9b91c652afb56c0b35a518dc81d06000400881be';
const git=(...a)=>cp.execFileSync('git',a,{encoding:'utf8'}).trim();
const head=git('rev-parse','HEAD'),apiTree=git('rev-parse','HEAD:api');
if(apiTree!==git('rev-parse',source+':api'))throw Error('API source mismatch');
const allowed=new Set(['.github/workflows/readonly-permissions-evidence.yml','.github/scripts/readonly-permissions-evidence.cjs']);
if(git('diff','--name-only',source,head).split('\n').filter(Boolean).some(p=>!allowed.has(p)))throw Error('Non-CI source delta');
const jobs=cp.execFileSync(process.execPath,['scripts/node/testing/verify-runtime.js','cargo-jobs'],{encoding:'utf8'}).trim();
const out=path.resolve('tmp/test-governance/readonly-permissions-ci');fs.mkdirSync(out,{recursive:true});
const suites=[
 ['console-navigation','-p','api-server','--lib','_tests::console_navigation_routes::'],
 ['settings-page-authority','-p','api-server','--lib','root_2014_ac_009_'],
 ['managed-service-mcp','-p','api-server','--lib','managed_mcp_'],
];
const receipt={source,head,apiTree,jobs:Number(jobs),started_at:new Date().toISOString(),results:[]};
const save=()=>fs.writeFileSync(out+'/results.json',JSON.stringify(receipt,null,2)+'\n');save();
for(const [name,...suite]of suites){
 const args=['--config','profile.test.package.api-server.debug=0','--config','profile.test.package.api-server.codegen-units=256','test','--locked','--manifest-path','api/Cargo.toml',...suite,'--jobs',jobs,'--','--test-threads='+jobs];
 const file=out+'/'+name+'.log',fd=fs.openSync(file,'w'),start=Date.now();
 const r=cp.spawnSync('cargo',args,{stdio:['ignore',fd,fd],timeout:50*60000});fs.closeSync(fd);
 const log=fs.readFileSync(file,'utf8'),counts=parseCargoTestCounts(log);let error=r.error?.message||null;
 try{requireExecutedCargoTests(log)}catch(e){error=e.message}
 const item={name,command:['cargo',...args],code:r.status,signal:r.signal,seconds:(Date.now()-start)/1000,counts,error};receipt.results.push(item);save();console.log(JSON.stringify(item));
 if(r.status!==0||error){process.exitCode=1;console.log(log.slice(-9000));if(!counts||counts.passedCount+counts.failedCount===0)break;}
}
receipt.finished_at=new Date().toISOString();receipt.pass=receipt.results.length===suites.length&&receipt.results.every(x=>x.code===0&&!x.error&&x.counts?.passedCount>0&&x.counts.failedCount===0);save();if(!receipt.pass)process.exitCode=1;
