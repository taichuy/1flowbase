'use strict';
function percentile(xs, q) { const a=xs.filter(Number.isFinite).sort((x,y)=>x-y); return a.length ? a[Math.ceil(a.length*q)-1] : null; }
function requestSummary(rows, seconds) {
 const good=rows.filter(r=>r.ok);
 return {attempted:rows.length,success:good.length,failed:rows.length-good.length,wall_seconds:seconds,success_per_second:seconds>0?good.length/seconds:null,ttft_ms:{n:rows.filter(r=>Number.isFinite(r.ttft_ms)).length,p50:percentile(rows.map(r=>r.ttft_ms),.5),p95:percentile(rows.map(r=>r.ttft_ms),.95),p99:percentile(rows.map(r=>r.ttft_ms),.99)},interdelta_gap_ms:{p50:percentile(rows.flatMap(r=>r.interdelta_gap_ms||[]),.5),p95:percentile(rows.flatMap(r=>r.interdelta_gap_ms||[]),.95)},latency_ms:{p50:percentile(rows.map(r=>r.elapsed_ms),.5),p95:percentile(rows.map(r=>r.elapsed_ms),.95),p99:percentile(rows.map(r=>r.elapsed_ms),.99)}};
}
function summarize(samples, ticks, attempted=0) {
 if(samples.length<2) throw Error('At least two boundary samples required');
 const seconds=(samples.at(-1).monotonic_ns-samples[0].monotonic_ns)/1e9;
 const groups={};
 for(const group of ['api','plugins','postgres','mock','driver','sampler']) {
  const initial=new Map(samples[0].processes.filter(p=>p.group===group).map(p=>[p.identity,p.cpu_ticks]));
  const final=new Map(initial); let unknown_births=0;
  for(const s of samples) for(const p of s.processes.filter(p=>p.group===group)) {
   if(!initial.has(p.identity)) { initial.set(p.identity,(Number.isFinite(p.start_ticks)&&p.start_ticks/ticks>=samples[0].monotonic_ns/1e9)?0:p.cpu_ticks); if(!(Number.isFinite(p.start_ticks)&&p.start_ticks/ticks>=samples[0].monotonic_ns/1e9)) unknown_births++; }
   final.set(p.identity,Math.max(final.get(p.identity)||0,p.cpu_ticks));
  }
  const cpu=[...final].reduce((n,[id,v])=>n+v-initial.get(id),0)/ticks;
  let peak=null; const intervals=[];
  for(let i=1;i<samples.length;i++) {
   const a=samples[i-1],b=samples[i], dt=(b.monotonic_ns-a.monotonic_ns)/1e9;
   const prev=new Map(a.processes.filter(p=>p.group===group).map(p=>[p.identity,p.cpu_ticks]));
   const delta=b.processes.filter(p=>p.group===group&&prev.has(p.identity)).reduce((n,p)=>n+Math.max(0,p.cpu_ticks-prev.get(p.identity)),0)/ticks;
   if(dt>0) { const v=100*delta/dt; intervals.push({seconds:dt,cpu_percent_single_core:v,valid_for_peak:dt>=.1,guard_min_interval_seconds:.1});if(dt>=.1)peak=peak===null?v:Math.max(peak,v); }
  }
  groups[group]={cpu_seconds:cpu,average_cpu_percent_single_core:100*cpu/seconds,sampled_peak_cpu_percent_single_core:peak,peak_scope:'sampled interval average; not theoretical instantaneous maximum; vanished-between-samples CPU can be missed',sample_count:samples.length,intervals,cpu_seconds_per_attempt:attempted?cpu/attempted:null,unknown_existing_births:unknown_births,rss_sample_peak_bytes:Math.max(...samples.map(s=>s.processes.filter(p=>p.group===group).reduce((n,p)=>n+(p.rss_bytes||0),0))),pss_sample_peak_bytes:Math.max(...samples.map(s=>s.processes.filter(p=>p.group===group).reduce((n,p)=>n+(p.pss_bytes||0),0)))};
 }
 return {wall_seconds:seconds,groups};
}
function history(bytes){if(!Number.isInteger(bytes)||bytes<512||bytes>524288)throw Error('history byte guard');return Array.from({length:128},(_,i)=>{const n=Math.floor(bytes/128)+(i<bytes%128?1:0);return {role:'user',content:[{type:'input_text',text:String(i).padStart(3,'0')+'|'+('h'.repeat(n-4))}]};});}
function profileOracle(d,warmed){const assert=require('node:assert/strict');assert.ok(Array.isArray(d?.runtime_targets)&&d.runtime_targets.length>=2);for(const t of d.runtime_targets){if(!t.reachable)continue;assert.ok(t.metrics);assert.ok(['available','warming_up','stale','unavailable'].includes(t.metrics.cpu.availability));if(warmed)assert.equal(t.metrics.cpu.availability,'available');if(t.metrics.cpu.availability==='available')assert.ok(t.metrics.cpu.limit_cores>0);}}
module.exports={summarize,requestSummary,history,profileOracle};
