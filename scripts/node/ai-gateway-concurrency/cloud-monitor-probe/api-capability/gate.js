'use strict';
const fs=require('node:fs'),path=require('node:path'),{spawn}=require('node:child_process');
const old=require('../observer-diagnostic/profile');
function inventory(target,io={read:p=>fs.readFileSync(p,'utf8'),list:p=>fs.readdirSync(p),uid:process.getuid()}){
 if(!Number.isInteger(target.pid)||target.pid<=1)throw Error('Owned API PID required');
 const base='/proc/'+target.pid,raw=io.read(base+'/stat'),fields=raw.slice(raw.lastIndexOf(')')+2).trim().split(/\s+/);
 const start=Number(fields[19]),parent=Number(fields[1]);
 if(parent!==target.parent||(target.start_ticks!==undefined&&start!==target.start_ticks))throw Error('API PID identity changed');
 const status=io.read(base+'/status'),uids=status.match(/^Uid:\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)/m);
 if(!uids||uids.slice(1).some(x=>Number(x)!==io.uid))throw Error('API UID differs from ordinary owner');
 const maps=io.read(base+'/maps'),thread_ids=io.list(base+'/task').map(Number).sort((a,b)=>a-b);
 const final=io.read(base+'/stat'),last=final.slice(final.lastIndexOf(')')+2).trim().split(/\s+/);
 if(Number(last[19])!==start||Number(last[1])!==parent)throw Error('API PID identity changed after inventory');
 return {pid:target.pid,identity:target.pid+':'+start,start_ticks:start,parent,uid:io.uid,maps,status:status.split('\n').filter(l=>/^(Name|Pid|PPid|NSpid|Uid):/.test(l)).join('\n'),thread_ids,group:'api'};
}
function perfArgs(pid,file){if(!Number.isInteger(pid)||pid<=1)throw Error('Owned API PID required');return ['record','-e','cpu-clock','-F','49','--call-graph','fp','-p',String(pid),'--no-inherit','-o',file];}
async function record(out,pid){
 const args=perfArgs(pid,path.join(out,'perf.data'));let stderr='',timer,killTimer;
 const begin=process.hrtime.bigint().toString();
 const result=await new Promise(resolve=>{const child=spawn('perf',args,{stdio:['ignore','ignore','pipe']});child.stderr.on('data',b=>stderr=(stderr+b).slice(-65536));child.once('error',e=>resolve({error:e.message,code:null}));child.once('close',(code,signal)=>resolve({code,signal}));timer=setTimeout(()=>{child.kill('SIGINT');killTimer=setTimeout(()=>child.kill('SIGKILL'),5000);},1000);});
 clearTimeout(timer);clearTimeout(killTimer);const receipt={args,...result,stderr,begin_ns:begin,end_ns:process.hrtime.bigint().toString(),probe_limit_ms:1000,identity:'ordinary process UID; no privilege changes'};
 fs.writeFileSync(path.join(out,'recorder.json'),JSON.stringify(receipt,null,2));fs.writeFileSync(path.join(out,'perf.stderr.log'),stderr);return receipt;
}
async function capability(out,target,binary,deps={}){
 fs.mkdirSync(out,{recursive:true});const receipt={api_file_access:'not_attempted',perf_record:'not_attempted',warm_requests:0,measured_requests:0,PG_inventory:'never_attempted',allowed:false};
 try{
  const owned=(deps.inventory||inventory)(target);receipt.api_file_access='pass';fs.writeFileSync(path.join(out,'owned-inventory.json'),JSON.stringify([owned],null,2));fs.writeFileSync(path.join(out,'ownership-maps.json'),JSON.stringify([owned],null,2));
  // Build-ID comes from the already sealed artifact; no /proc/exe alternate route.
  const elf=(deps.spawnSync||require('node:child_process').spawnSync)('readelf',['-n',binary],{encoding:'utf8',maxBuffer:1024*1024});receipt.ELF={status:elf.status,stdout:elf.stdout,stderr:elf.stderr,error:elf.error?.message};if(elf.status!==0)throw Error('sealed artifact ELF unavailable');
  receipt.facts=(deps.facts||old.facts)();fs.writeFileSync(path.join(out,'capability-facts.json'),JSON.stringify(receipt.facts,null,2));if(receipt.facts.version.status!==0)throw Error('existing perf unavailable');
  const recorded=await (deps.record||record)(out,target.pid);receipt.perf_record=recorded.code===0?'permission_pass':'failed';receipt.recorder=recorded;if(recorded.code!==0){receipt.reason='STOP actual API perf recorder failure; no fallback';return receipt;}
  const decoded=(deps.decode||old.decode)(out);receipt.decode=decoded;receipt.thread_coverage={inventory_threads:owned.thread_ids,sampled_threads:[...new Set(decoded.samples.filter(Boolean).map(s=>s.tid))],new_threads_not_inherited:true,coverage:'PID thread enumeration at attach; idle snapshot does not prove later thread coverage'};receipt.kernel_symbols='restricted or unknown symbols retained; API system CPU not assigned from user leaves';receipt.allowed=decoded.decode_ok&&decoded.known_owned_leaf_samples>0&&decoded.parsed_samples===decoded.sample_headers&&decoded.samples.filter(Boolean).every(s=>s.owned);
  receipt.reason=receipt.allowed?'API permission and attributable idle leaf observed; return gate before original budget':'recorder permission passed; idle symbol attribution insufficient, no forced requests';
 }catch(e){receipt.reason='STOP '+e.message;receipt.error_code=e.code;receipt.failed_path=e.path;if(receipt.api_file_access==='not_attempted')receipt.api_file_access='failed';}
 return receipt;
}
module.exports={inventory,perfArgs,capability};
