const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),crypto=require('node:crypto');const {spawnSync}=require('node:child_process');
const out=path.resolve('tmp/test-governance/record-permissions-ci');fs.mkdirSync(out,{recursive:true});
const source=spawnSync('git',['rev-parse','HEAD'],{encoding:'utf8'});if(source.status)throw Error('Candidate identity unavailable');fs.writeFileSync(path.join(out,'candidate-sha.txt'),source.stdout.trim()+'\n');
const env={...process.env,CARGO_BUILD_JOBS:String(os.availableParallelism())},receipts=[];
function run(name,args,minimum=0){const file=path.join(out,name+'.log'),fd=fs.openSync(file,'w'),start=Date.now();let result;try{result=spawnSync('cargo',args,{env,stdio:['ignore',fd,fd]})}finally{fs.closeSync(fd)}const log=fs.readFileSync(file,'utf8'),rows=[...log.matchAll(/test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored/g)],counts=rows.reduce((a,m)=>a.map((n,i)=>n+Number(m[i+1])),[0,0,0]);const ok=result.status===0&&counts[0]>=minimum&&counts[1]===0&&counts[2]===0;const receipt={name,command:['cargo',...args],exit_code:result.status,passed:counts[0],failed:counts[1],ignored:counts[2],minimum,duration_ms:Date.now()-start,ok};receipts.push(receipt);console.log(JSON.stringify(receipt));if(!ok)process.stderr.write(log.slice(-6500));fs.writeFileSync(path.join(out,'receipt.json'),JSON.stringify(receipts,null,2));return ok}
const base=['test','--locked','--manifest-path','api/Cargo.toml'];
run('domain',[...base,'-p','domain','--lib','record_permissions::tests::'],3);
run('manifest',[...base,'-p','extension-package-runtime','--lib','_tests::managed_service_tests::record_'],4);
run('worker-frame',[...base,'-p','extension-contracts','--lib','worker_cannot_supply_record_permission_context_in_frame_or_data_request'],1);
run('policy-service',[...base,'-p','control-plane','--lib','record_permissions::tests::'],6);
run('postgres-data',[...base,'-p','storage-durable-postgres','--lib','_tests::plugin::plugin_data_repository_tests::','--','--test-threads=1'],15);
run('worker-context',[...base,'-p','runtime-extension-host','--lib','resource_replay_retains_worker_scope_without_restoring_recursive_callback_delegation'],1);
if(run('runtime-fixture',['build','--locked','--manifest-path','api/Cargo.toml','-p','runtime-extension-sdk','--example','record_permission_worker']))env.RECORD_PERMISSION_WORKER_FIXTURE=path.resolve('api/target/debug/examples/record_permission_worker');
run('record-migration',[...base,'-p','api-server','--test','api_private_contracts','record_permission_migration'],2);
run('canonical-managed',[...base,'-p','api-server','--lib','managed_services::','--','--test-threads=1','--skip','record_permission_runtime'],1);
run('record-runtime',[...base,'-p','api-server','--lib','record_permission_runtime','--','--include-ignored','--test-threads=1'],2);
if(run('runtime-build',['build','--locked','--manifest-path','api/Cargo.toml','-p','api-server','--bin','api-server'])){const binary=path.join(out,'api-server'),r=spawnSync('strip',['-o',binary,'api/target/debug/api-server'],{encoding:'utf8'});if(r.status)throw Error(r.stderr);const hash=crypto.createHash('sha256'),fd=fs.openSync(binary,'r'),buf=Buffer.alloc(128*1024);try{let n;while((n=fs.readSync(fd,buf,0,buf.length,null))>0)hash.update(buf.subarray(0,n))}finally{fs.closeSync(fd)}fs.writeFileSync(path.join(out,'runtime-sha256.txt'),hash.digest('hex')+'\n')}
process.exitCode=receipts.every(r=>r.ok)?0:1;
