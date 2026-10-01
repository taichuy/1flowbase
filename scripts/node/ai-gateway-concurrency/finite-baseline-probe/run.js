#!/usr/bin/env node
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { loadSingleErrorRow, tracedFetch } = require('./adapter');
const BASELINE = '59ff883cd772df01e29a16ab1bf68962f758bb38';
const OFFICIAL = 'a927373b69d2109e32bf9d43c545a89b19a7dea1';
const root = path.resolve(process.env.PROBE_REPO_ROOT || process.cwd());
const suite = process.argv[2];
if (!['fidelity-wire', 'ws-json'].includes(suite)) throw new Error('Unknown finite probe suite');
const out = path.join(root, 'tmp/test-governance/gateway-finite-baseline', suite);
fs.mkdirSync(out, { recursive: true });
const frozen = name => require(path.join(root, 'scripts/node/ai-gateway-concurrency', name));
const { createDatabase } = frozen('local-acceptance/system');
const { createGatewayFixture } = frozen('gateway-fixture');
const { createMockUpstream } = frozen('mock-upstream');
const { singlePackage } = frozen('workflow-contract/inputs');
const { redactServiceLog } = frozen('gateway-fixture/service-logs');
const packages = path.join(root, 'tmp/protocol-reuse/ai-gateway-quality-gate/packages');
let secrets = ['sk-1flowbase-controlled-secret-canary', 'fixture-openai-token', 'fixture-anthropic-token', 'fixture-openai_compatible-token'];
const redact = value => redactServiceLog(String(value), secrets);
const write = (name, value) => fs.writeFileSync(path.join(out, name), redact(JSON.stringify(value, null, 2)) + '\n');
const checks = [];
const http = [];
const originalFetch = globalThis.fetch;
let db = null, mock = null, fixture = null;

(async () => {
  const sha = spawnSync('git', ['rev-parse','HEAD'], {cwd:root,encoding:'utf8'});
  if (sha.status !== 0 || sha.stdout.trim() !== BASELINE) throw new Error('Frozen baseline checkout mismatch');
  const lock = frozen('workflow-contract/paired-source.lock.json');
  if (lock.official_plugins.revision !== OFFICIAL) throw new Error('Paired official lock mismatch');
  const containers = spawnSync('docker',['ps','--filter','publish=5432','--format','{{.ID}}'],{encoding:'utf8'});
  const ids = containers.stdout?.trim().split(/\s+/);
  if (containers.status !== 0 || ids.length !== 1 || !ids[0]) throw new Error('Expected one PostgreSQL service');
  db = createDatabase({container:ids[0],host:'127.0.0.1',port:5432});
  secrets.push(db.url, new URL(db.url).password);
  mock = createMockUpstream({slowChunkDelayMs:40});
  const endpoints = await mock.start();
  const packagePaths = {openai:singlePackage(path.join(packages,'openai'),'openai'),anthropic:singlePackage(path.join(packages,'anthropic'),'anthropic'),openai_compatible:singlePackage(path.join(packages,'openai_compatible'),'openai_compatible')};
  fixture = await createGatewayFixture({databaseUrl:db.url,apiServerBin:path.resolve(root,process.env.PROBE_BINARY || 'api/target/debug/api-server'),frontstageCompilerRoot:path.join(root,'web'),openaiPackage:packagePaths.openai,anthropicPackage:packagePaths.anthropic,openaiCompatiblePackage:packagePaths.openai_compatible,upstreamBaseUrl:endpoints.httpBaseUrl,artifactRoot:out});
  const ready = fixture.result;
  for (const target of Object.values(ready.targets)) secrets.push(target.api_key, target.durable.list_runs.headers.cookie);
  for (const target of ready.pools.anthropic) secrets.push(target.api_key);
  write('provenance.json', {baseline:BASELINE,official:OFFICIAL,suite,profile:'test',candidate_comparison:'67a78a8828d79678d53e0551f8815b20447eddc2',candidate_run:36837561449,packages:ready.packages});
  globalThis.fetch = tracedFetch(originalFetch, http, redact);
  const attempt = async (name, work) => {
    const row = {name,start_ns:process.hrtime.bigint().toString()}; checks.push(row);
    try {row.result = await work();row.status='pass';} catch(error){row.status='fail';row.error=redact(error.message);}
    row.end_ns=process.hrtime.bigint().toString(); write('results.json',{suite,checks});
  };
  if (suite === 'fidelity-wire') {
    const {verifyGatewayRequestFidelity} = frozen('workflow-contract/request-fidelity-gateway');
    const {runWireAudit} = frozen('wire-audit/runner');
    await attempt('request-fidelity',()=>verifyGatewayRequestFidelity({ready,upstreamBaseUrl:endpoints.httpBaseUrl,mockSnapshot:mock.snapshot}));
    const c=ready.controlled_upstream;
    await attempt('wire-audit',()=>runWireAudit({manifest:{gatewayBaseUrl:ready.gateway_base_url,openai:ready.targets.openai,anthropic:ready.targets.anthropic,controlledUpstream:{snapshotUrl:c.snapshot_url,barrierReleaseUrl:c.barrier_release_url,networkObserverUrl:c.network_observer_url,gatewayExecutorObserverUrl:c.gateway_executor_observer_url}}},{secretCanary:'sk-1flowbase-controlled-secret-canary'}));
  } else {
    const {runGatewayWebSocketAcceptance} = frozen('workflow-contract/gateway-websocket');
    await attempt('responses-websocket',()=>runGatewayWebSocketAcceptance({ready,mockSnapshot:mock.snapshot}));
    const {runGatewayErrorMatrix} = loadSingleErrorRow(root);
    await attempt('json/responses-sse',()=>runGatewayErrorMatrix({ready,mockSnapshot:mock.snapshot}).then(result=>{
      if(result.rows.length!==1 || result.rows[0].id!=='json/responses-sse') throw new Error('Finite inventory mismatch');
      if(result.verdict!=='PASS') {const e=new Error(result.rows[0].error || 'JSON error row failed');e.result=result;write('json-error-row.json',result);throw e;}
      return result;
    }));
  }
})().catch(error=>{checks.push({name:'fixture-or-infrastructure',status:'fail',error:redact(error.message)});}).finally(async()=>{
  globalThis.fetch=originalFetch;
  write('http-trace.json',http);
  if(mock) write('mock-timeline.json',mock.snapshot().entries.map(({sequence,event,transport,nonce,scenario,monotonic_ns,status,outcome})=>({sequence,event,transport,nonce,scenario,monotonic_ns,status,outcome})));
  for(const [name,close] of [['fixture',()=>fixture?.close()],['mock',()=>mock?.stop()],['database',()=>db?.close()]]){
    try {await close();} catch(error){checks.push({name:`cleanup-${name}`,status:'fail',error:redact(error.message)});}
  }
  write('results.json',{suite,checks});
  process.stdout.write(redact(JSON.stringify({suite,checks:checks.map(({name,status,error})=>({name,status,error}))}))+'\n');
  process.exitCode = checks.some(x=>x.status==='fail') || checks.length!==2 ? 1 : 0;
});
