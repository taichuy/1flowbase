#!/usr/bin/env node
'use strict';
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {execFileSync}=require('node:child_process'),{capability}=require('./gate');
const ROOT=process.cwd(),OUT=path.join(ROOT,'tmp/test-governance/gateway-api-capability'),freeze=require('./freeze.json');
const frozen=x=>require(path.join(ROOT,'scripts/node/ai-gateway-concurrency',x));
const {createGatewayFixture}=frozen('gateway-fixture'),{createDatabase}=frozen('local-acceptance/system'),{singlePackage}=frozen('workflow-contract/inputs');
const {OwnerHttpClient}=frozen('gateway-fixture/http-owner'),{openTemporaryOwnerSession}=require(path.join(ROOT,'scripts/node/page-debug/auth'));
const {redactServiceLog}=frozen('gateway-fixture/service-logs');
const command=(exe,args)=>execFileSync(exe,args,{encoding:'utf8',maxBuffer:1024*1024}).trim();
async function main(){
 fs.mkdirSync(OUT,{recursive:true});const verified=JSON.parse(fs.readFileSync(path.join(OUT,'build/verified-inputs.json')));
 const manifest={freeze,harness_sha:process.env.WORKFLOW_SHA,verified,logical_cpu_count:require('node:os').cpus().length,CPU_accounting:'one CPU second = one core second; idle gate has no request denominator',sample_hz:49,probe_limit_ms:1000,model_requests:0,warm_requests:0,measured_requests:0,direct_requests:0,native_model_requests:0,real_model_requests:0,scope:'A zero-load gate only; return evidence before any original Stage2 budget',PG_profile:'STOP prior EACCES; no maps/exe access or profiling'};
 fs.writeFileSync(path.join(OUT,'manifest.json'),JSON.stringify(manifest,null,2));
 const result={status:'initializing',cleanup:[],stageB:'not_started',stageC:'not_selected',warm_requests:0,measured_requests:0,direct_requests:0,native_model_requests:0};
 let fixture,database,requests=0;const sessions=[],secrets=[];
 // An inert mock never emits model data; any unexpected request fails zero-load acceptance.
 const mock=http.createServer((req,res)=>{requests++;req.resume();res.writeHead(503);res.end('zero-load capability gate');});
 class Client extends OwnerHttpClient{async signIn(account,password){const s=await openTemporaryOwnerSession({apiBaseUrl:this.baseUrl,account,password});sessions.push(s);this.attachSession(s.cookie,s.csrfToken);}}
 try{
  await new Promise(r=>mock.listen(0,'127.0.0.1',r));
  const container=command('docker',['ps','--filter','publish=5432','--format','{{.ID}}']);assert.match(container,/^[a-f0-9]+$/);
  database=createDatabase({container,host:'127.0.0.1',port:5432});secrets.push(database.url,new URL(database.url).password);
  const packageRoot=path.join(ROOT,'tmp/protocol-reuse/ai-gateway-quality-gate/packages');
  const packages=Object.fromEntries(['openai','anthropic','openai_compatible'].map(k=>[k,singlePackage(path.join(packageRoot,k),k)]));
  const binary=path.join(OUT,'build/baseline-release-api-server');fs.chmodSync(binary,0o700);
  fixture=await createGatewayFixture({databaseUrl:database.url,apiServerBin:binary,openaiPackage:packages.openai,anthropicPackage:packages.anthropic,openaiCompatiblePackage:packages.openai_compatible,upstreamBaseUrl:'http://127.0.0.1:'+mock.address().port,artifactRoot:path.join(OUT,'service')},{OwnerHttpClient:Client});
  assert.ok(!['7600','7800'].includes(new URL(fixture.result.gateway_base_url).port));
  for(const target of Object.values(fixture.result.targets))secrets.push(target.api_key,target.durable.list_runs.headers.cookie);
  assert.equal(requests,0,'No upstream/model request during bootstrap');
  result.api_pid=fixture.gatewayPid;result.capability=await capability(path.join(OUT,'capability'),{pid:fixture.gatewayPid,parent:process.pid},binary);
  assert.equal(requests,0,'Zero upstream/model requests through capability gate');
  result.status=result.capability.allowed?'gate_pass_return_before_load':'STOP_capability';
 }catch(e){result.status='fixture_or_boundary_failure';result.error=redactServiceLog(e.message,secrets);process.exitCode=1;}
 finally{
  for(const s of sessions)try{await s.dispose();result.cleanup.push('session-disposed');}catch{result.cleanup.push('session-dispose-failed');process.exitCode=1;}
  for(const [name,close]of [['fixture',()=>fixture?.close()],['mock',()=>new Promise(r=>{mock.closeAllConnections();mock.close(r);})],['database',()=>database?.close()]])try{await close();result.cleanup.push(name+'-closed');}catch(e){result.cleanup.push(name+'-cleanup-failed');result.cleanup_error=redactServiceLog(e.message,secrets);process.exitCode=1;}
  result.mock_requests=requests;if(requests!==0){result.status='zero_request_boundary_failed';process.exitCode=1;}fs.writeFileSync(path.join(OUT,'summary.json'),redactServiceLog(JSON.stringify({manifest,result,complete:true},null,2),[...secrets,...sessions.map(s=>s.cookie)]));
 }
}
main().catch(e=>{console.error(e.message);process.exitCode=1;});
