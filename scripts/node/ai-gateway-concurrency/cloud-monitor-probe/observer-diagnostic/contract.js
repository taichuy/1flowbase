'use strict';
const assert=require('node:assert/strict');
const GROUPS=['api','plugins','postgres','mock','driver','sampler','observer_helpers'];
async function observerSlot({enabled,name,slotMs=1500,clock=()=>performance.now(),wait=ms=>new Promise(r=>setTimeout(r,ms)),mark,suite}){
 const begin=await mark(name+':slot:begin'),slotStart=clock(),suiteBegin=await mark(name+':suite:begin'),suiteStart=clock(),cpuStart=process.cpuUsage();
 let value=null,error;
 try{if(enabled)value=await suite();}catch(e){error=e;}
 const driverCPU=process.cpuUsage(cpuStart),suiteMs=clock()-suiteStart,suiteEnd=await mark(name+':suite:end');
 const elapsed=clock()-slotStart,overrun=elapsed>slotMs;
 if(!overrun)await wait(slotMs-elapsed);
 const end=await mark(name+':slot:end'),receipt={name,sql_observer_enabled:enabled,sql_measured:enabled,slot_budget_ms:slotMs,slot_wall_ms:clock()-slotStart,suite_wall_ms:suiteMs,driver_user_cpu_us:driverCPU.user,driver_system_cpu_us:driverCPU.system,begin_ns:begin.monotonic_ns,suite_begin_ns:suiteBegin.monotonic_ns,suite_end_ns:suiteEnd.monotonic_ns,end_ns:end.monotonic_ns,overrun,helper_CPU_complete:false,helper_scope:'Short-lived docker/psql/backends may vanish between proc marks; captured CPU cannot be assumed complete.'};
 if(overrun||error){const failure=error||Object.assign(new Error('observer slot overrun '+name),{code:'OBSERVER_SLOT_OVERRUN'});failure.receipt=receipt;throw failure;}
 return {value,receipt};
}
function verifyFrozenInputs(f,actual){assert.equal(actual.source,f.product_source);assert.equal(actual.apiTree,f.product_api_tree);assert.equal(actual.binarySha,f.baseline_binary_sha256);assert.equal(actual.profile,f.profile);assert.equal(actual.features,f.features);assert.deepEqual(actual.packages,f.provider_packages_sha256);return true;}
function stage1Eligible(rows,f){return rows.length===4&&rows.every((r,i)=>r.label===f.stage1.order[i]&&r.status==='pass'&&r.paired_window_valid===true&&r.foreground_complete===true&&r.sql_observer_enabled===r.label.startsWith('SQL_ON'));}
function profileGate(p){return ['tool','permission','symbols','owned'].every(k=>p[k]===true)&&p.mode==='fp';}
function diagnosticMetrics(samples,ticks){
 if(samples.length<2)throw Error('Two diagnostic boundary samples required');
 const seconds=(samples.at(-1).monotonic_ns-samples[0].monotonic_ns)/1e9,groups={};
 for(const group of GROUPS){
  const rows=samples.map(s=>s.processes.filter(p=>p.group===group)),ids=[...new Set(rows.flatMap(r=>r.map(p=>p.identity)))],per=[];
  for(const id of ids){const observations=rows.map(r=>r.find(p=>p.identity===id)),a=observations[0],b=observations.at(-1),missing=observations.some(p=>!p),issues=missing?['identity_missing_at_boundary_or_interior']:[];const values={};
   for(const field of ['cpu_ticks','cpu_user_ticks','cpu_system_ticks']){const valid=!missing&&observations.every(p=>Number.isFinite(p[field]))&&observations.every((p,i)=>!i||p[field]>=observations[i-1][field]);values[field]=valid?(b[field]-a[field])/ticks:null;if(!valid)issues.push('invalid_'+field);}
   const io={};for(const field of ['rchar','wchar','syscr','syscw','read_bytes','write_bytes','cancelled_write_bytes']){const valid=!missing&&observations.every(p=>Number.isFinite(p.io?.[field]))&&observations.every((p,i)=>!i||p.io[field]>=observations[i-1].io[field]);io[field]=valid?b.io[field]-a.io[field]:null;}
   per.push({identity:id,pid:observations.find(Boolean).pid,cpu_seconds:values.cpu_ticks,cpu_user_seconds:values.cpu_user_ticks,cpu_system_seconds:values.cpu_system_ticks,io_delta:io,issues});
  }
  const sum=field=>per.length&&per.every(p=>p[field]!==null)?per.reduce((n,p)=>n+p[field],0):null;
  const memory=field=>rows.every(r=>r.every(p=>Number.isFinite(p[field])))?Math.max(...rows.map(r=>r.reduce((n,p)=>n+p[field],0))):null;
  let peak=null;for(let i=1;i<samples.length;i++){const dt=(samples[i].monotonic_ns-samples[i-1].monotonic_ns)/1e9;const a=new Map(rows[i-1].map(p=>[p.identity,p]));const valid=rows[i].every(p=>a.has(p.identity)&&p.cpu_ticks>=a.get(p.identity).cpu_ticks);if(dt>=.1&&valid){const v=100*rows[i].reduce((n,p)=>n+p.cpu_ticks-a.get(p.identity).cpu_ticks,0)/ticks/dt;if(!peak||v>peak.single_core_percent)peak={single_core_percent:v,interval_seconds:dt,start_ns:samples[i-1].monotonic_ns,end_ns:samples[i].monotonic_ns};}}
  const io_delta={};for(const field of ['rchar','wchar','syscr','syscw','read_bytes','write_bytes','cancelled_write_bytes'])io_delta[field]=per.length&&per.every(p=>p.io_delta[field]!==null)?per.reduce((n,p)=>n+p.io_delta[field],0):null;
  const cpu=sum('cpu_seconds');groups[group]={io_delta,cpu_seconds:cpu,cpu_user_seconds:sum('cpu_user_seconds'),cpu_system_seconds:sum('cpu_system_seconds'),captured_cpu_seconds_lower_bound:per.reduce((n,p)=>n+(p.cpu_seconds||0),0),identity_complete:per.length>0&&per.every(p=>!p.issues.length),cpu_average_single_core_percent:cpu===null?null:100*cpu/seconds,sampled_cpu_peak:peak,rss_peak_bytes:memory('rss_bytes'),pss_peak_bytes:memory('pss_bytes'),processes:per,unobserved_short_lived_processes_possible:true};
 }
 return {wall_seconds:seconds,sample_count:samples.length,groups,scope:'Observed process identities only; complete recorded boundaries do not exclude invisible short-lived processes.'};
}
module.exports={observerSlot,verifyFrozenInputs,stage1Eligible,profileGate,diagnosticMetrics,GROUPS};
