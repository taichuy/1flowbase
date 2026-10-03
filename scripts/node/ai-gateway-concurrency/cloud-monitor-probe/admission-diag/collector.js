'use strict';
const assert=require('node:assert/strict');
const STAGES=['request_admission','response_sse_admission','response_json_admission','archive_collect','archive_commit','replay_read','decode','request_classify_inclusive','response_classify_inclusive','fact_append','unbound_discard','complete_wait','worker_total'];
const UUID=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const exact=(value,keys)=>{assert.ok(value&&typeof value==='object'&&!Array.isArray(value));assert.deepEqual(Object.keys(value).sort(),keys.slice().sort());};
function validatePayload(p){
 exact(p,['version','event','capture_id','flow_run_id','ok','clock_reads','stages']);assert.equal(p.version,1);assert.ok(['worker_end','complete_end'].includes(p.event));assert.match(p.capture_id,UUID);if(p.flow_run_id!==null)assert.match(p.flow_run_id,UUID);assert.equal(typeof p.ok,'boolean');assert.ok(Number.isSafeInteger(p.clock_reads)&&p.clock_reads>=0);exact(p.stages,STAGES);
 for(const stage of Object.values(p.stages)){exact(stage,['count','total_ns','max_ns','failures','cancelled']);for(const value of Object.values(stage))assert.ok(Number.isSafeInteger(value)&&value>=0,'diagnostic counter not exactly representable');assert.ok(stage.max_ns<=stage.total_ns);assert.ok(stage.failures+stage.cancelled<=stage.count);if(!stage.count)assert.equal(stage.total_ns,0);}
 assert.ok(Buffer.byteLength(JSON.stringify(p))<=4096,'diagnostic payload budget');return p;
}
class Collector{
 constructor({eventLimit=128,lineByteLimit=8192}={}){this.eventLimit=eventLimit;this.lineByteLimit=lineByteLimit;this.lines=new Map();this.events=[];this.errors=[];this.unrelated=0;this.diagBytes=0;}
 feed(bytes,pipe='stdout'){
  assert.ok(['stdout','stderr'].includes(pipe),'owned API pipe only');let state=this.lines.get(pipe);if(!state){state={pending:'',discarding:false};this.lines.set(pipe,state);}
  // Attached only to the single owned API process. Never retain unrelated log text.
  const text=Buffer.isBuffer(bytes)?bytes.toString('utf8'):String(bytes);for(const part of text.split(/(?<=\n)/)){
   if(state.discarding){if(part.endsWith('\n'))state.discarding=false;continue;}
   state.pending+=part;if(Buffer.byteLength(state.pending)>this.lineByteLimit){if(state.pending.includes('gateway capture cumulative timing'))this.errors.push('owned API diagnostic log line budget exceeded');else this.unrelated++;state.pending='';state.discarding=!part.endsWith('\n');continue;}
   if(!part.endsWith('\n'))continue;const line=state.pending.replace(/\x1b\[[0-9;]*m/g,'');state.pending='';
   if(!line.includes('gateway capture cumulative timing')){this.unrelated++;continue;}
   try{const start=line.indexOf('diagnostic=');assert.ok(start>=0,'missing diagnostic field');const jsonStart=line.indexOf('{',start),jsonEnd=line.lastIndexOf('}');assert.ok(jsonStart>=0&&jsonEnd>=jsonStart,'missing diagnostic JSON');const p=validatePayload(JSON.parse(line.slice(jsonStart,jsonEnd+1)));assert.ok(this.events.length<this.eventLimit,'owned synthetic diagnostic event budget');this.events.push(p);this.diagBytes+=Buffer.byteLength(JSON.stringify(p));}catch(error){this.errors.push(error.message);}
  }
 }
 select(runIds,enabled){
  assert.equal(this.errors.length,0,'diagnostic collector errors: '+this.errors.join(';'));assert.equal(new Set(runIds).size,runIds.length,'duplicate measured UUID');for(const id of runIds)assert.match(id,UUID);if(!enabled){assert.equal(this.events.length,0,'OFF unexpectedly emitted instrumentation');return{enabled:false,rows:[],exported_events:0,diagnostic_json_bytes:0,unrelated_lines:this.unrelated};}
  const rows=runIds.map(flow=>{const events=this.events.filter(p=>p.flow_run_id===flow),captures=new Set(events.map(p=>p.capture_id));assert.equal(captures.size,1,'one measured capture per flow required');assert.equal(events.length,2,'exact worker_end and first complete_end required');const worker=events.find(p=>p.event==='worker_end'),complete=events.find(p=>p.event==='complete_end');assert.ok(worker&&complete);assert.ok(worker.ok&&complete.ok,'measured capture completion failure');for(const p of [worker,complete])for(const[name,stage]of Object.entries(p.stages)){assert.equal(stage.failures,0,'failed stage '+name);assert.equal(stage.cancelled,0,'cancelled stage '+name);}assert.equal(complete.stages.worker_total.count,1);assert.ok(complete.stages.complete_wait.count>=1);assert.ok(complete.stages.archive_commit.count>0);assert.ok(complete.stages.fact_append.count>0);assert.ok(complete.stages.request_admission.count>0);assert.ok(complete.stages.response_sse_admission.count>0);return{flow_run_id:flow,capture_id:complete.capture_id,worker,complete};});
  return{enabled:true,rows,exported_events:rows.length*2,diagnostic_json_bytes:Buffer.byteLength(JSON.stringify(rows)),unrelated_lines:this.unrelated,observed_total_events:this.events.length,observed_total_diagnostic_json_bytes:this.diagBytes,snapshot_boundary:'worker_end emitted after stopped/notify; complete_end after first complete waiter returns; concurrent repeat waiters may race counters, not atomically joint snapshots'};
 }
}
module.exports={Collector,STAGES,validatePayload};
