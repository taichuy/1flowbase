'use strict';
const fs=require('node:fs'),cp=require('node:child_process'),path=require('node:path');
const {parseCargoTestCounts,requireExecutedCargoTests}=require('../../scripts/node/verify/cargo-test-results.js');
const source='c378e1d9d2232edc0e846706c4bbad89dfd3895f';
const git=(...a)=>cp.execFileSync('git',a,{encoding:'utf8'}).trim();
const head=git('rev-parse','HEAD'),apiTree=git('rev-parse','HEAD:api');
if(apiTree!==git('rev-parse',source+':api'))throw Error('API source mismatch');
const allowed=new Set(['.github/workflows/readonly-permissions-evidence.yml','.github/scripts/readonly-permissions-evidence.cjs']);
if(git('diff','--name-only',source,head).split('\n').filter(Boolean).some(p=>!allowed.has(p)))throw Error('Non-CI source delta');
const jobs=cp.execFileSync(process.execPath,['scripts/node/testing/verify-runtime.js','cargo-jobs'],{encoding:'utf8'}).trim();
const out=path.resolve('tmp/test-governance/readonly-permissions-ci');fs.mkdirSync(out,{recursive:true});
const suites=[
 ['system-view-role','-p','control-plane','--lib','workspace_role_system_all'],
 ['role-routes','-p','api-server','--lib','role_routes'],
 ['notification-routes','-p','api-server','--lib','_tests::notifications::'],
 ['personal-routes','-p','api-server','--lib','_tests::me_routes::'],
 ['notification-migration','-p','api-server','--lib','communication_operations_preserve_authenticated_access'],
 ['route-ownership','-p','api-server','--lib','console_route_assembly_tests'],
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
