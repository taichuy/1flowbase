'use strict';
// Root #2028 AC-006/009/011/013/014. Run from tmux with TEST_KEY_FILE and
// EVIDENCE_DIR; artifacts contain structural facts only, never raw credentials.
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const http = require('node:http');
const cp = require('node:child_process');
const repo = process.env.ONEFLOW_REPO || '/home/taichuy/git/1flowbase';
const pnpmRoot=path.join(repo,'web/node_modules/.pnpm');
const wsPackage=fs.readdirSync(pnpmRoot).filter(name=>/^ws@8\./.test(name)).sort().at(-1);
if(!wsPackage) throw Error('Installed ws@8 package is required');
const { WebSocket, WebSocketServer } = require(path.join(pnpmRoot,wsPackage,'node_modules/ws'));
const targetPort = Number(process.argv[2] || 7801);
const count = Number(process.argv[3] || 1);
const label = process.argv[4] || 'candidate';
const mode = process.argv[5] || 'normal';
if (!['normal','disconnect','worker-restart'].includes(mode) || (mode!=='normal' && count!==1)) throw Error('unsupported recovery fixture');
const evidenceDir = process.env.EVIDENCE_DIR;
if (!evidenceDir || !process.env.TEST_KEY_FILE || ![1, 2].includes(count)) throw Error('EVIDENCE_DIR, TEST_KEY_FILE and count 1/2 required');
const key = fs.readFileSync(process.env.TEST_KEY_FILE, 'utf8').trim();
const privateRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'responses-2028-chain-'));
fs.chmodSync(privateRoot, 0o700);
const catalog = JSON.parse(fs.readFileSync(process.env.MODEL_CATALOG_FILE || '/home/taichuy/.codex/models_cache.json', 'utf8'));
fs.writeFileSync(path.join(privateRoot, 'models.json'), JSON.stringify({ models: catalog.models }));
const sockets = new Set();
const children = new Set();
const relays = [];
const rows = [];
const barrier = { expected: count, connected: new Set(), release: [], released: false };
let timedOut = false;
function arrive(id, release) {
  if (barrier.released) return release();
  barrier.connected.add(id); barrier.release.push(release);
  if (barrier.connected.size === count) {
    barrier.released = true;
    rows.forEach(row => row.barrier = { simultaneouslyConnectedClients: count, released: true });
    barrier.release.splice(0).forEach(fn => fn());
  }
}
function clean() {
  for (const child of children) { try { process.kill(-child.pid, 'SIGTERM'); } catch {} }
  for (const socket of sockets) socket.terminate();
  for (const {server,wss} of relays) { wss.close(); server.close(); }
  fs.rmSync(privateRoot, { recursive: true, force: true });
}
function observeRequest(row, raw) {
  const frame = JSON.parse(raw);
  const inputs = Array.isArray(frame.input) ? frame.input : [];
  row.requests.push({type:frame.type, generate:frame.generate, model:frame.model, previous_response_id:frame.previous_response_id,
    inputTypes:inputs.map(v=>v.type || 'message'), resultCallIds:inputs.filter(v=>/^(custom_tool_call_output|function_call_output)$/.test(v.type)).map(v=>v.call_id),
    resultDigests:inputs.filter(v=>/^(custom_tool_call_output|function_call_output)$/.test(v.type)).map(v=>crypto.createHash('sha256').update(JSON.stringify(v.output)).digest('hex'))});
}
function observeResponse(row, raw) {
  const frame = JSON.parse(raw);
  const item = frame.item;
  row.wire.push({type:frame.type, sequence:frame.sequence_number, response_id:frame.response?.id || frame.response_id,
    output_index:frame.output_index, item_id:item?.id || frame.item_id, item_type:item?.type, phase:item?.phase,
    call_id:item?.call_id || frame.call_id, name:item?.name, inputType:typeof item?.input, argumentsType:typeof item?.arguments,
    hasEncryptedContent:typeof item?.encrypted_content==='string', errorCode:frame.error?.code || frame.response?.error?.code});
  if (frame.error || frame.response?.error) row.errors.push({type:frame.type, code:frame.error?.code || frame.response?.error?.code});
}
async function run(id) {
  const nonce = crypto.randomBytes(24).toString('hex');
  const next1 = `step-${crypto.randomBytes(12).toString('hex')}.txt`;
  const next2 = `step-${crypto.randomBytes(12).toString('hex')}.txt`;
  const workspace = path.join(privateRoot, `client-${id}`); fs.mkdirSync(workspace);
  fs.writeFileSync(path.join(workspace,'entry.txt'), `Read ${next1} in a new tool call.\n`);
  fs.writeFileSync(path.join(workspace,next1), `Read ${next2} in a new tool call.\n`);
  fs.writeFileSync(path.join(workspace,next2), `FINAL=${nonce}\n`);
  const row = {id, targetPort, requests:[], wire:[], errors:[], client:[]}; rows.push(row);
  const server = http.createServer((req,res)=>{row.errors.push({type:'unexpected_http_fallback'});res.writeHead(409);res.end('This controlled WebSocket fixture does not hide transport fallback');});
  const wss = new WebSocketServer({noServer:true}); relays.push({server,wss});
  server.on('upgrade',(req,socket,head)=>{
    const headers={...req.headers};
    for(const name of ['host','connection','upgrade','sec-websocket-key','sec-websocket-version','sec-websocket-extensions','sec-websocket-protocol']) delete headers[name];
    const upstream=new WebSocket(`ws://127.0.0.1:${targetPort}${req.url}`,{headers}); sockets.add(upstream);
    upstream.on('error',()=>{row.errors.push({type:'upstream_websocket_error'});socket.destroy();});
    upstream.on('open',()=>wss.handleUpgrade(req,socket,head,client=>{
      sockets.add(client);
      let released=false; const pending=[];
      client.on('message',(raw,binary)=>{
        if(!binary)observeRequest(row,raw.toString());
        const hasResult=!binary && row.requests.at(-1).resultCallIds.length>0;
        if(mode!=='normal' && !row.recoveryInjected && hasResult) {
          row.recoveryInjected=true;
          row.completedToolIdsBeforeRecovery=row.wire.filter(v=>v.type==='response.output_item.done'&&v.call_id).map(v=>v.call_id);
          if(mode==='worker-restart') {
            const pid=Number(process.env.CANDIDATE_WORKER_PID), parent=Number(process.env.CANDIDATE_SERVER_PID);
            if(targetPort!==7801 || !pid || !parent || !fs.readFileSync(`/proc/${pid}/status`,'utf8').includes(`PPid:\t${parent}\n`)) {
              row.errors.push({type:'worker_identity_not_verified'});client.close();return;
            }
            process.kill(pid,'SIGKILL');
            // Deliver the owned cursor once so the client sees the actual worker
            // generation failure. Its bounded retry must submit full history.
          } else {upstream.close();client.close();return;}
        }
        const send=()=>upstream.send(raw,{binary});if(released)send();else pending.push(send);
      });
      upstream.on('message',(raw,binary)=>{if(!binary)observeResponse(row,raw.toString());if(client.readyState===WebSocket.OPEN)client.send(raw,{binary});});
      client.on('close',()=>upstream.close()); upstream.on('close',()=>client.close());
      arrive(id,()=>{released=true;pending.splice(0).forEach(fn=>fn());});
    }));
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const base=`http://127.0.0.1:${server.address().port}/v1`;
  const args=['exec','--ignore-user-config','--ignore-rules','--ephemeral','--skip-git-repo-check','--sandbox','read-only','--json','--color','never','-C',workspace,'-m','gpt-5.6-terra'];
  const config={model_provider:'native_acceptance',model_reasoning_effort:'low',model_catalog_json:path.join(privateRoot,'models.json'),
    'model_providers.native_acceptance.name':'native acceptance','model_providers.native_acceptance.base_url':base,
    'model_providers.native_acceptance.wire_api':'responses','model_providers.native_acceptance.env_key':'ONEFLOW_PROBE_API_KEY',
    'model_providers.native_acceptance.supports_websockets':true,'features.responses_websockets_v2':true,
    'features.multi_agent':false,'features.unbounded_connection_retries':false,
    'model_providers.native_acceptance.stream_max_retries':mode==='normal'?0:2,'model_providers.native_acceptance.request_max_retries':0,
    'shell_environment_policy.exclude':['ONEFLOW_PROBE_API_KEY']};
  for(const [name,value]of Object.entries(config))args.push('-c',`${name}=${JSON.stringify(value)}`);
  args.push('Read-only protocol acceptance task. Use the terminal tool to run `cat entry.txt`. That file names another file to read. Follow the chain using exactly one cat command per tool call, waiting for each result before issuing the next call. Do not combine commands, list directories, inspect other files, use scripts, modify files, or spawn agents. After three separate dependent tool calls, the last file gives FINAL=<value>. Reply with only that value, verbatim, without formatting.');
  const child=cp.spawn('codex',args,{env:{...process.env,ONEFLOW_PROBE_API_KEY:key},stdio:['ignore','pipe','pipe'],detached:true});children.add(child);
  let output='',stderr='';child.stdout.on('data',chunk=>output+=chunk);child.stderr.on('data',chunk=>stderr=(stderr+chunk).slice(-5000));
  const code=await new Promise(resolve=>child.on('close',resolve));children.delete(child);row.exitCode=code;
  const parsed=output.split('\n').flatMap(line=>{try{return[JSON.parse(line)];}catch{return[];}});
  const answer=parsed.filter(e=>e.type==='item.completed'&&e.item?.type==='agent_message').at(-1)?.item?.text;
  row.finalAnswerMatches=typeof answer==='string'&&answer.trim()===nonce;
  row.expectedDigest=crypto.createHash('sha256').update(nonce).digest('hex');
  row.client=parsed.map(e=>({type:e.type,itemType:e.item?.type,status:e.item?.status,exitCode:e.item?.exit_code,
    command:e.item?.command?.replaceAll(workspace,'<fixture>'),errorType:e.error?.type}));
  const tools=row.wire.filter(e=>e.type==='response.output_item.done'&&['custom_tool_call','function_call'].includes(e.item_type));
  row.toolRounds=row.requests.filter(r=>r.resultCallIds.length>0).length;
  row.uniqueToolIds=new Set(tools.map(e=>e.call_id)).size;
  row.allToolResultsReturned=tools.every(t=>row.requests.some(r=>r.resultCallIds.includes(t.call_id)));
  const completedCommands=row.client.filter(e=>e.type==='item.completed'&&e.itemType==='command_execution').map(e=>e.command);
  row.noRepeatedCompletedTools=new Set(completedCommands).size===completedCommands.length;
  row.passed=(mode==='normal'||row.recoveryInjected)&&row.noRepeatedCompletedTools&&code===0&&row.finalAnswerMatches&&row.toolRounds>=3&&row.uniqueToolIds>=3&&row.allToolResultsReturned&&(mode==='worker-restart'
    ? row.errors.length>0&&row.errors.every(e=>['error','response.failed'].includes(e.type))
    : row.errors.length===0);
  row.stderr=stderr.replaceAll(key,'[REDACTED]').replaceAll(privateRoot,'<private>');
  return row;
}
(async()=>{
  const timer=setTimeout(()=>{timedOut=true;clean();},180000);
  try {
    await Promise.all(Array.from({length:count},(_,index)=>run(index)));
    const responseSets=rows.map(row=>new Set(row.wire.map(w=>w.response_id).filter(Boolean)));
    const isolated=count===1 || [...responseSets[0]].every(id=>!responseSets[1].has(id));
    const result={label,mode,barrierReleased:barrier.released,timedOut,isolated,passed:!timedOut&&isolated&&rows.every(r=>r.passed),rows};
    fs.mkdirSync(evidenceDir,{recursive:true});
    fs.writeFileSync(path.join(evidenceDir,`${label}.json`),JSON.stringify(result,null,2).replaceAll(key,'[REDACTED]'));
    console.log(JSON.stringify({label,passed:result.passed,isolated,rows:rows.map(r=>({id:r.id,toolRounds:r.toolRounds,finalAnswerMatches:r.finalAnswerMatches,errors:r.errors}))}));
    process.exitCode=result.passed?0:1;
  } finally {clearTimeout(timer);clean();}
})().catch(()=>{clean();console.error('Acceptance harness failed; sensitive details withheld');process.exitCode=1;});
