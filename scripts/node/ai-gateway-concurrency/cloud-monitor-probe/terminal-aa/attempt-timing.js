'use strict';
const assert=require('node:assert/strict');
const UUID=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const STAGES=['ingress','mapping','flow','queue','connect','upstream','flush'];
const SUMMARY=['schema_version','capture_mode','event_count','total_size_bytes','first_ingress_ms','last_ingress_ms','max_append_delay_ms','event_kind_counts'];
function targets({database,applicationId,runIds}){
 assert.match(database,/^qadb[a-f0-9]+$/);assert.match(applicationId,UUID);
 assert.ok(Array.isArray(runIds)&&runIds.length===8&&new Set(runIds).size===8,'exact eight unique measured IDs');
 for(const id of runIds)assert.match(id,UUID);return{database,applicationId,runIds};
}
function sql(input){
 const{applicationId,runIds}=targets(input);const object=(base,keys)=>`jsonb_build_object(${keys.map(k=>`'${k}', ${base}->'${k}'`).join(',')})`;
 const stages=`jsonb_build_object(${STAGES.map(k=>`'${k}', ${object(`a->'provider_timing_receipt'->'stages'->'${k}'`,['owner','duration_ms','availability','unavailable_reason'])}`).join(',')})`;
 const receipt=`jsonb_build_object('schema_version',a->'provider_timing_receipt'->'schema_version','attempt_index',a->'provider_timing_receipt'->'attempt_index','termination_kind',a->'provider_timing_receipt'->'termination_kind','stages',${stages},'connection',${object(`a->'provider_timing_receipt'->'connection'`,['cold','reused','generation','close_acknowledged','physical_state'])})`;
 // The database exports only host-owned numeric timing/enum fields, never the full metrics document.
 return `SELECT coalesce(jsonb_agg(jsonb_build_object('run_id',n.flow_run_id::text,'node_run_id',n.id::text,'application_id',f.application_id::text,'attempts',coalesce((SELECT jsonb_agg(jsonb_build_object('attempt_index',a->'attempt_index','status',a->'status','started_at',a->'started_at','first_token_at',a->'first_token_at','finished_at',a->'finished_at','time_to_first_token_ms',a->'time_to_first_token_ms','provider_stream_timing_summary',CASE WHEN a ? 'provider_stream_timing_summary' THEN ${object(`a->'provider_stream_timing_summary'`,SUMMARY)} ELSE NULL END,'provider_timing_receipt',CASE WHEN a ? 'provider_timing_receipt' THEN ${receipt} ELSE NULL END)) FROM jsonb_array_elements(n.metrics_payload->'attempts') a),'[]'::jsonb)) ORDER BY n.flow_run_id,n.id),'[]'::jsonb) FROM node_run_records n JOIN flow_runs f ON f.id=n.flow_run_id WHERE f.application_id='${applicationId}'::uuid AND f.id IN (${runIds.map(x=>`'${x}'::uuid`).join(',')}) AND jsonb_typeof(n.metrics_payload->'attempts')='array'`;
}
function pick(value,keys){return Object.fromEntries(keys.filter(k=>value?.[k]!==undefined).map(k=>[k,value[k]]));}
function summary(value){
 if(!value||value.schema_version!==1||value.capture_mode!=='summary')return null;
 if(!['event_count','total_size_bytes'].every(k=>Number.isSafeInteger(value[k])&&value[k]>=0)||value.event_count===0)return null;
 if(!['first_ingress_ms','last_ingress_ms','max_append_delay_ms'].every(k=>Number.isSafeInteger(value[k])&&value[k]>=0))return null;
 if(value.last_ingress_ms<value.first_ingress_ms)return null;
 if(!value.event_kind_counts||typeof value.event_kind_counts!=='object'||Array.isArray(value.event_kind_counts))return null;
 if(!Object.entries(value.event_kind_counts).every(([k,v])=>/^[a-z_]{1,64}$/.test(k)&&Number.isSafeInteger(v)&&v>=0))return null;
 return pick(value,SUMMARY);
}
function rows(value,input){
 const{applicationId,runIds}=targets(input);assert.ok(Array.isArray(value)&&value.length<=8,'bounded current fixture node records');const seen=new Set();
 const result=value.map(row=>{
  assert.ok(runIds.includes(row.run_id),'foreign/unmeasured run ID');assert.equal(row.application_id,applicationId);assert.match(row.node_run_id,UUID);assert.ok(!seen.has(row.run_id),'one controlled LLM node per measured run');seen.add(row.run_id);
  assert.ok(Array.isArray(row.attempts)&&row.attempts.length<=1,'no implicit model retry');
  const attempts=row.attempts.map(a=>{
   const out=pick(a,['attempt_index','status','started_at','first_token_at','finished_at','time_to_first_token_ms']);assert.equal(a.attempt_index,0);assert.equal(a.status,'succeeded');
   out.provider_stream_timing_summary=summary(a.provider_stream_timing_summary);
   const r=a.provider_timing_receipt;out.provider_timing_receipt=r?{...pick(r,['schema_version','attempt_index','termination_kind']),stages:Object.fromEntries(STAGES.map(k=>[k,pick(r.stages?.[k],['owner','duration_ms','availability','unavailable_reason'])])),connection:pick(r.connection,['cold','reused','generation','close_acknowledged','physical_state'])}:null;
   return out;
  });
  return{run_id:row.run_id,node_run_id:row.node_run_id,application_id:row.application_id,attempts};
 });
 return{rows:result,missing_run_ids:runIds.filter(id=>!seen.has(id)),all_summaries_available:result.length===8&&result.every(r=>r.attempts.length===1&&r.attempts[0].provider_stream_timing_summary!==null),
  server_inner_body_eof:null,server_recorder_complete:null,dispatcher_span:null,boundary:'phase-outside single read of current owned fixture measured run IDs; existing host timing only, no full metrics/body'};
}
module.exports={targets,sql,rows,summary,STAGES,SUMMARY};
