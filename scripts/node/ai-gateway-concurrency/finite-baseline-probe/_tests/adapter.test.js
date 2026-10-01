'use strict';
const test=require('node:test');const assert=require('node:assert/strict');const path=require('node:path');
const {loadSingleErrorRow,tracedFetch}=require('../adapter');
const root=path.resolve(__dirname,'../../../../..');
const {upstreamErrorFixture}=require('../../protocol-oracle/error-fidelity');
async function run(message){
  let entries=[];const fixture=upstreamErrorFixture('json');
  return loadSingleErrorRow(root).runGatewayErrorMatrix({ready:{targets:{openai:{}}},mockSnapshot:()=>({entries})},{
    observeClient:async(surface)=>{assert.equal(surface,'responses-sse');entries=[{sequence:1,event:'arrival',nonce:'n'},{sequence:2,event:'error',errorFixture:'json',status:500}];return {http_status:200,records:[{data:{type:'error',error:{message:fixture.body}}}]};},
    observeRun:async()=>({run_id:'r',native:{status:'failed',error:{message}},durable:{status:'failed',error_payload:{message}}}),
  });
}
test('selector retains one JSON SSE row and original fidelity checks',async()=>{const r=await run(upstreamErrorFixture('json').body);assert.equal(r.verdict,'PASS');assert.deepEqual(r.rows.map(x=>x.id),['json/responses-sse']);});
test('selector still rejects a wrong durable error body',async()=>{const r=await run('wrong');assert.equal(r.verdict,'FAIL');assert.match(r.rows[0].error,/preserve exact upstream body/);});
test('trace preserves returned response and redacts failure body without recording headers',async()=>{const records=[];const response=new Response('secret',{status:500});const f=tracedFetch(async()=>response,records,s=>s.replaceAll('secret','[REDACTED]'),()=> '1');const returned=await f('http://127.0.0.1:123/list?token=secret',{headers:{authorization:'secret'}});assert.equal(returned,response);assert.equal(await returned.text(),'secret');assert.equal(records[0].error_body,'[REDACTED]');assert.equal(records[0].url,'http://127.0.0.1:123/list');assert.equal(JSON.stringify(records).includes('secret'),false);});
