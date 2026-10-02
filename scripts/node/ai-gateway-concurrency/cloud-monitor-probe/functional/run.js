#!/usr/bin/env node
'use strict';
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),assert=require('node:assert/strict'),{execFileSync}=require('node:child_process');
const ROOT=process.cwd(),OUT=path.join(ROOT,'tmp/test-governance/gateway-fixed-functional'),REUSE=path.join(ROOT,'tmp/reused-release');
const BASELINE='80b4e635e1738e1e2df1a22f94dfd7a6bc9a31af',CANDIDATE='8ec70687f4b2ec74e79f69bef0e15fe3f12aae7d',PROVIDER='997c705b3c564c93d2fc49b973e0b5bbdb759cdc';
const expected={baseline:'07e34e2d6e906bfc2c8fc4ffe5d191353f3f5e276b670eab98c6eba9edcf5630',candidate:'329ff4ecb953968eb4da787f99c577dac15559cdca2888d23414007841e2b883',openai:'8d5a338447566d8ac26a2572c2d536fad37838e766572df4dd23686965b57b3e',anthropic:'07af38ee7aff8ebd59894f5884a1c5cb7861248a87710e1437d13db429b79642',openai_compatible:'33c99266767d7d8cbe8b27c639065d5eeda854453dc8c9cc04c7ac8f64db0516'};
const frozen=x=>require(path.join(ROOT,'scripts/node/ai-gateway-concurrency',x));
const {createGatewayFixture}=frozen('gateway-fixture'),{createDatabase}=frozen('local-acceptance/system'),{singlePackage}=frozen('workflow-contract/inputs'),{createMockUpstream}=frozen('mock-upstream');
const {OwnerHttpClient}=frozen('gateway-fixture/http-owner'),{spawnOwned}=frozen('gateway-fixture/process-owner'),{redactServiceLog}=frozen('gateway-fixture/service-logs');
const {openTemporaryOwnerSession}=require(path.join(ROOT,'scripts/node/page-debug/auth'));
const {runGatewayWebSocketLifecycle}=require('../../responses-websocket-acceptance/lifecycle');
const {runGatewayErrorMatrix,observeClient}=require('../../responses-websocket-acceptance/error-matrix');
const {collectGatewayFrames}=require('../../workflow-contract/gateway-websocket');
const {createGatewayTarget}=require('../../responses-websocket-acceptance/target');
const {assertUniqueProjection}=require('./protocol-checks');
const command=(bin,args)=>execFileSync(bin,args,{encoding:'utf8',maxBuffer:1024*1024}).trim(),sha=p=>crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
fs.mkdirSync(OUT,{recursive:true});assert.equal(command('git',['rev-parse','HEAD']),CANDIDATE);
const build=path.join(REUSE,'test-governance/gateway-duplicate-run-lock/build'),bins={},packages={};
for(const k of ['baseline','candidate']){bins[k]=path.join(build,k+'-release-api-server');assert.equal(sha(bins[k]),expected[k]);fs.chmodSync(bins[k],0o700);}
for(const k of ['openai','anthropic','openai_compatible']){packages[k]=singlePackage(path.join(REUSE,'protocol-reuse/ai-gateway-quality-gate/packages',k),k);assert.equal(sha(packages[k]),expected[k]);}
const source=JSON.parse(fs.readFileSync(path.join(build,'binaries.json')));assert.equal(source.baseline.source,BASELINE);assert.equal(source.candidate.source,CANDIDATE);
const provider=JSON.parse(fs.readFileSync(path.join(REUSE,'protocol-reuse/ai-gateway-concurrency/provider-provenance.json')));assert.equal(provider.official_source_sha,PROVIDER);
const container=command('docker',['ps','--filter','publish=5432','--format','{{.ID}}']);assert.match(container,/^[a-f0-9]+$/);
const lifecycleIds=['client-disconnect','native-cancel','upstream-interruption'];
fs.writeFileSync(path.join(OUT,'manifest.json'),JSON.stringify({baseline:BASELINE,candidate:CANDIDATE,provider:PROVIDER,workflow:process.env.GITHUB_SHA,release_artifact_run:36967545246,release_artifact_id:11211672582,release_artifact_digest:'47b028e1d51a791c4c136dc6b6529f4c7965458f9d3d039721a7e9a29947302b',hashes:expected,sequence:['baseline','candidate'],lifecycleIds,errorRows:['retry/responses-websocket'],concurrency:1,performance_measurement:false,limits:['No real CLI client or model','Synthetic tool executes once; resume submits full history on a fresh WebSocket','No assertion of all async backlog flush']},null,2));
async function toolRecovery(ready){
 const target=createGatewayTarget(ready),input=[{role:'user',content:[{type:'input_text',text:'1flowbase-client-tool-vector TOOL_VECTOR_PATH=synthetic-fixture.txt'}]}],tools=[{type:'function',name:'fixture_command',description:'Synthetic read-only fixture',parameters:{type:'object',properties:{command:{type:'string'}},required:['command']}}];
 const firstObs={},firstFrames=await collectGatewayFrames(target,'tool-first-'+crypto.randomUUID(),{inputItems:input,requestFields:{tools},observation:firstObs});
 const first=firstFrames.map(x=>JSON.parse(Buffer.concat(x).toString())),projection=assertUniqueProjection(first);
 assert.equal(projection.completed_tool_calls,1,'Exactly one tool from first turn');
 const call=first.find(x=>x.type==='response.output_item.done'&&x.item?.type==='function_call')?.item;assert.ok(call);
 // Collector closes the first socket before callback submission. Do not retry
 // the synthetic tool; reconnect once with the original completed pair/history.
 const execution={call_id:call.call_id,executions:1,result:'1flowbase-client-tool-result',at_ns:process.hrtime.bigint().toString()};
 const history=[...input,call,{type:'function_call_output',call_id:call.call_id,output:execution.result}],nextObs={};
 const nextFrames=await collectGatewayFrames(target,'tool-recovery-'+crypto.randomUUID(),{inputItems:history,requestFields:{tools},observation:nextObs});
 const next=nextFrames.map(x=>JSON.parse(Buffer.concat(x).toString())),final=assertUniqueProjection(next,{text:'1flowbase gateway tool sentinel ok'});assert.equal(final.completed_tool_calls,0,'No repeated tool after full-history resume');
 assert.ok(BigInt(firstObs.closed_ns)<BigInt(nextObs.opened_ns),'Closed socket precedes fresh connection');
 return {verdict:'PASS',mode:'socket closed between tool response and result; explicit fresh WS full-history resume, not real CLI automatic retry',execution,first:firstObs,recovered:nextObs,first_projection:projection,recovered_projection:final};
}
async function run(arm){
 const out=path.join(OUT,arm);fs.mkdirSync(out,{recursive:true});const result={arm,source:arm==='baseline'?BASELINE:CANDIDATE,status:'FAIL',cleanup:[]},sessions=[],secrets=[];let fixture,db,mock;
 const save=()=>fs.writeFileSync(path.join(out,'result.json'),redactServiceLog(JSON.stringify(result,null,2),secrets)+'\n');
 class Client extends OwnerHttpClient{async signIn(account,password){const s=await openTemporaryOwnerSession({apiBaseUrl:this.baseUrl,account,password});sessions.push(s);this.attachSession(s.cookie,s.csrfToken);}}
 try{
  db=createDatabase({container,host:'127.0.0.1',port:5432});secrets.push(db.url,new URL(db.url).password);mock=createMockUpstream({slowChunkDelayMs:25});const endpoints=await mock.start();
  fixture=await createGatewayFixture({databaseUrl:db.url,apiServerBin:bins[arm],openaiPackage:packages.openai,anthropicPackage:packages.anthropic,openaiCompatiblePackage:packages.openai_compatible,upstreamBaseUrl:endpoints.httpBaseUrl,artifactRoot:path.join(out,'service')},{OwnerHttpClient:Client,spawnOwned});
  const ready=fixture.result;assert.notEqual(new URL(ready.gateway_base_url).port,'7600');for(const t of Object.values(ready.targets))secrets.push(t.api_key,t.durable.list_runs.headers.cookie);secrets.push(...sessions.map(x=>x.cookie));
  result.lifecycle=await runGatewayWebSocketLifecycle({ready,mockSnapshot:mock.snapshot,terminalBarriers:mock.terminalBarriers,selectedRows:lifecycleIds});save();assert.equal(result.lifecycle.verdict,'PASS');assert.deepEqual(result.lifecycle.rows.map(x=>x.id),lifecycleIds);
  result.retry=await runGatewayErrorMatrix({ready,mockSnapshot:mock.snapshot,selectedRows:['retry/responses-websocket']},{observeClient:async(...args)=>{const x=await observeClient(...args);const success=x.records.some(r=>r.data?.type==='response.completed');assertUniqueProjection(x.records.map(r=>r.data).filter(Boolean),{terminal:success?'response.completed':'response.failed'});return x;}});save();assert.equal(result.retry.verdict,'PASS');assert.equal(result.retry.rows.length,1);assert.equal(result.retry.rows[0].attempts.length,2);
  result.tool_recovery=await toolRecovery(ready);save();result.mock=mock.snapshot();result.status='PASS';
 }catch(e){result.error=redactServiceLog(e.stack||e.message,secrets);}finally{
  for(const s of sessions)try{await s.dispose();result.cleanup.push('session-disposed');}catch{result.status='FAIL';result.cleanup.push('session-failed');}
  for(const[name,close]of[['fixture',()=>fixture?.close()],['mock',()=>mock?.stop()],['database',()=>db?.close()]])try{await close();result.cleanup.push(name+'-closed');}catch{result.status='FAIL';result.cleanup.push(name+'-failed');}
  save();console.log(JSON.stringify({arm,status:result.status,error:result.error?.slice(0,1500),cleanup:result.cleanup}));
 }
 return result;
}
(async()=>{const rows=[];for(const arm of ['baseline','candidate'])rows.push(await run(arm));const result={status:rows.every(x=>x.status==='PASS')?'PASS':'FAIL',rows:rows.map(x=>({arm:x.arm,status:x.status}))};fs.writeFileSync(path.join(OUT,'summary.json'),JSON.stringify(result,null,2));if(result.status!=='PASS')process.exitCode=1;})().catch(e=>{console.error(e.message);process.exitCode=1;});
