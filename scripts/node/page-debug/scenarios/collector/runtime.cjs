'use strict';
const fs = require('node:fs'), path = require('node:path'), crypto = require('node:crypto');
const assert = require('node:assert/strict');
const { spawn, execFileSync } = require('node:child_process');
const { pathToFileURL } = require('node:url');
const root = path.resolve(__dirname, '../../../../..');
const pluginsRoot = process.env.COLLECTOR_PLUGINS_ROOT;
const binary = process.env.COLLECTOR_API_BINARY;
const nativeBinary = process.env.FLOWBASE_COLLECTOR_TEST_BINARY;
const out = process.env.COLLECTOR_EVIDENCE_DIR;
const base = 'http://127.0.0.1:7801', web = 'http://127.0.0.1:3102', dbPort = 35783;
const container = 'codex-platform-collector-proof';
const image = process.env.COLLECTOR_PG_IMAGE || 'sha256:b07129cc272f688c98f5b343138a0a52fa45b3d82f50d7a53ff441330624cd2e';
const { parseEnvFile } = require('../../../dev-up/env.js');
const { openTemporaryOwnerSession } = require('../../auth.js');
const password = crypto.randomBytes(24).toString('hex');
let api, frontend, child, owner, containerId, timer, remote, resourceBreach = false;
const receipt = { checks: [], cleanup: {}, peaks: { cpu: 0, physical: 0, disk: 0 } };
const protectedPids = [7800,3100].map(port => {
  const pids = execFileSync('fuser', ['-n','tcp',String(port)], {encoding:'utf8',stdio:['ignore','pipe','ignore']}).trim().split(/\s+/).map(Number);
  return pids.map(pid => ({ pid, start: fs.readFileSync(`/proc/${pid}/stat`,'utf8').split(' ')[21], cwd: fs.readlinkSync(`/proc/${pid}/cwd`) }));
}).flat();
function dock(args) { return execFileSync('docker', args, {encoding:'utf8',maxBuffer:1024*1024,stdio:['ignore','pipe','ignore']}).trim(); }
function start(cmd,args,cwd,name,env) {
  const fd = fs.openSync(path.join(out,`${name}.log`),'w');
  const processOwner = spawn(cmd,args,{cwd,env,detached:true,stdio:['ignore',fd,fd]}); fs.closeSync(fd); return processOwner;
}
async function stop(p) {
  if (!p || p.exitCode !== null) return;
  try { process.kill(-p.pid,'SIGTERM'); } catch {}
  await Promise.race([new Promise(resolve=>p.once('exit',resolve)),new Promise(resolve=>setTimeout(resolve,5000))]);
  if (p.exitCode === null) try { process.kill(-p.pid,'SIGKILL'); } catch {}
}
async function ready(url, ms=90000) {
  const until = Date.now()+ms;
  while (Date.now()<until) {
    if (resourceBreach) throw Error('resource bound breached');
    try { if ((await fetch(url,{signal:AbortSignal.timeout(2000)})).ok) return; } catch {}
    await new Promise(resolve=>setTimeout(resolve,500));
  }
  throw Error('readiness timeout '+url);
}
async function request(method, route, data, authenticated=true, language='zh-Hans') {
  const response = await fetch(base+route,{signal:AbortSignal.timeout(30000),method,headers:{'content-type':'application/json','accept-language':language,...(authenticated?{cookie:owner.cookie,'x-csrf-token':owner.csrfToken}:{})},...(data===undefined?{}:{body:JSON.stringify(data)})});
  let body; try { body=await response.json(); } catch {}
  return {status:response.status,body};
}
async function run(cmd,args,name,env) {
  child=start(cmd,args,root,name,env);
  const exit=await new Promise((resolve,reject)=>{child.once('error',reject);child.once('exit',resolve);});
  child=null; receipt.checks.push({name,exit}); assert.equal(exit,0,`${name} failed; see ${name}.log`);
}
(async()=>{try {
  assert.ok(pluginsRoot && binary && nativeBinary && out,'explicit proof roots, binaries and evidence dir required'); fs.mkdirSync(out,{recursive:true});
  receipt.source_sha=execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim();
  if(process.env.COLLECTOR_EXPECTED_SHA) assert.equal(receipt.source_sha,process.env.COLLECTOR_EXPECTED_SHA);
  receipt.plugins_sha=execFileSync('git',['rev-parse','HEAD'],{cwd:pluginsRoot,encoding:'utf8'}).trim();
  receipt.binary_sha256=crypto.createHash('sha256').update(fs.readFileSync(binary)).digest('hex');
  for(const port of [7801,3102,dbPort]) assert.equal(execFileSync('ss',['-H','-ltn','sport','=',':'+port],{encoding:'utf8'}).trim(),'');
  const {createRemoteFixture}=await import(pathToFileURL(path.join(__dirname,'remote-fixture.mjs')));
  remote=await createRemoteFixture({pluginsRoot,directory:path.join(out,'private/remote'),nativeBinary});
  const env={...process.env,...parseEnvFile('/home/taichuy/git/1flowbase/api/apps/api-server/.env'),
    API_DATABASE_URL:`postgres://postgres:${password}@127.0.0.1:${dbPort}/collector_proof`,API_SERVER_ADDR:'127.0.0.1:7801',API_NODE_ID:'collector-proof-node',
    BOOTSTRAP_ROOT_ACCOUNT:'collector-proof-owner',BOOTSTRAP_ROOT_PASSWORD:password,API_ENV:'development',
    API_PROVIDER_INSTALL_ROOT:path.join(out,'private/providers'),API_HOST_EXTENSION_DROPIN_ROOT:path.join(out,'private/dropins'),
    API_SYSTEM_BACKUP_REPOSITORY_ROOT:path.join(out,'private/backups'),API_MCP_TEMPLATE_LIBRARY_ROOT:path.join(out,'private/mcp'),
    API_OFFICIAL_PLUGIN_DEFAULT_REGISTRY_URL:remote.baseUrl+'/official-registry.json',API_OFFICIAL_PLUGIN_MIRROR_REGISTRY_URL:'',API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL:'',
    API_OFFICIAL_PLUGIN_TRUSTED_PUBLIC_KEYS_JSON:remote.trustedKeys,API_OFFICIAL_EXTENSION_CATALOG_MIRROR_BASE_URL:remote.baseUrl,
    VITE_DEV_SERVER_PORT:'3102',VITE_API_PROXY_TARGET:base,VITE_API_BASE_URL:'',VITE_DEV_CACHE_DIR:path.join(root,'web/app/.vite/collector-distribution-proof'),pnpm_config_verify_deps_before_run:'false'};
  delete env.DATABASE_URL;
  let previous=fs.readFileSync('/proc/stat','utf8').split('\n')[0].split(/\s+/).slice(1).map(Number);
  timer=setInterval(()=>{
    const next=fs.readFileSync('/proc/stat','utf8').split('\n')[0].split(/\s+/).slice(1).map(Number),delta=next.reduce((a,b)=>a+b,0)-previous.reduce((a,b)=>a+b,0);
    const cpu=100*(delta-(next[3]-previous[3])-(next[4]-previous[4]))/Math.max(1,delta); previous=next;
    const mem=Object.fromEntries(fs.readFileSync('/proc/meminfo','utf8').trim().split('\n').map(line=>{const[k,v]=line.split(':');return[k,parseInt(v)];}));
    const physical=100*(mem.MemTotal-mem.MemAvailable)/mem.MemTotal, s=fs.statfsSync(root), disk=100*(s.blocks-s.bfree)/s.blocks;
    for(const[k,v]of Object.entries({cpu,physical,disk})) receipt.peaks[k]=Math.max(receipt.peaks[k],v);
    if(Math.max(cpu,physical,disk)>=90&&!resourceBreach){resourceBreach=true;void stop(child);void stop(api);void stop(frontend);}
  },2000);
  containerId=dock(['run','-d','--name',container,'--label','codex.qa.owner='+receipt.source_sha,'--publish',`127.0.0.1:${dbPort}:5432`,'--env','POSTGRES_PASSWORD='+password,'--env','POSTGRES_DB=collector_proof',image]);
  receipt.database={id:containerId,name:container,port:dbPort,scope:'fresh proof-owned database only'};
  for(let i=0;i<60;i++){try{dock(['exec',container,'pg_isready','-h','127.0.0.1','-U','postgres','-d','collector_proof']);break;}catch{}if(i===59)throw Error('PG readiness failed');await new Promise(resolve=>setTimeout(resolve,500));}
  api=start(binary,[],root+'/api','api',env); frontend=start(process.execPath,[root+'/web/app/node_modules/vite/bin/vite.js','--host','127.0.0.1','--port','3102','--strictPort'],root+'/web/app','web',env);
  await ready(base+'/health'); await ready(web+'/__1flowbase_dev_ready');
  receipt.runtime={api_pid:api.pid,web_pid:frontend.pid,api_port:7801,web_port:3102};
  owner=await openTemporaryOwnerSession({apiBaseUrl:base,account:env.BOOTSTRAP_ROOT_ACCOUNT,password});
  assert.equal((await request('GET','/api/console/applications/catalog',undefined,false)).status,401);
  const before=await request('GET','/api/console/applications/catalog');assert.equal(before.status,200);
  const pending=before.body.data.collectors.find(item=>item.collector_code==='codex-logs-collector');assert.ok(pending);
  assert.equal(pending.installation_status,'not_installed');assert.equal(pending.can_install,true);assert.equal(pending.asset_base_url,null);
  const app=await request('POST','/api/console/applications',{application_type:'agent_logs',name:'Platform collector proof',description:'proof owned',icon:null,icon_type:null,icon_background:null});assert.equal(app.status,201);
  const applicationId=app.body.data.id;receipt.application_id=applicationId;
  // Real browser mutation installs package. Desktop installs, mobile reads retained state.
  await run(process.execPath,['scripts/node/page-debug/scenarios/collector.cjs'],'browser',{...env,COLLECTOR_WEB_BASE_URL:web,COLLECTOR_API_BASE_URL:base,COLLECTOR_APPLICATION_ID:applicationId,COLLECTOR_EVIDENCE_DIR:path.join(out,'browser'),COLLECTOR_EXPECTED_ENDPOINT:web+'/api/logs/v1/events'});
  const after=await request('GET','/api/console/applications/catalog');assert.equal(after.status,200);
  const installed=after.body.data.collectors.find(item=>item.catalog_id===pending.catalog_id);
  assert.equal(installed.installation_status,'installed');assert.equal(installed.installed_version,'0.1.0');assert.ok(installed.extension_installation_id);
  const installInput={category:installed.category,catalog_id:installed.catalog_id,version:installed.installed_version};
  remote.disable();const remoteCount=remote.requests.length;
  const offlineExisting=await request('POST','/api/console/settings/extension-center/install',installInput);assert.equal(offlineExisting.status,200);
  assert.equal(offlineExisting.body.data.local_artifact_was_present,true);assert.equal(offlineExisting.body.data.node_plugin_installation_id,null);
  const assetReceipt=[];
  for(const asset of remote.manifest.assets){
    const response=await fetch(base+installed.asset_base_url+'/'+asset.name,{redirect:'error'});assert.equal(response.status,200);assert.equal(response.headers.get('location'),null);
    const bytes=Buffer.from(await response.arrayBuffer());assert.equal(bytes.length,asset.size);assert.equal(crypto.createHash('sha256').update(bytes).digest('hex'),asset.sha256);
    assetReceipt.push({name:asset.name,bytes:bytes.length,sha256:asset.sha256});
  }
  assert.equal(remote.requests.length,remoteCount,'asset serving and existing install must not contact remote source');
  for(const suffix of ['/not-whitelisted','/%2e%2e%2fsecret','/%2fetc%2fpasswd']){
    const response=await fetch(base+installed.asset_base_url+suffix);assert.ok(response.status>=400);
  }
  for(const route of [installed.asset_base_url.replace('/0.1.0/','/9.9.9/')+'/install.sh',installed.asset_base_url.replace('/taichuy/','/other/')+'/install.sh'])assert.ok((await fetch(base+route)).status>=400);
  const installer=await fetch(base+installed.shell_installer_url);assert.equal(installer.status,200);
  const localScript=path.join(out,'private/install.sh');fs.writeFileSync(localScript,await installer.text());
  await run('bash',[localScript,'--endpoint',base+'/api/logs/v1/events','--release-base',base+installed.asset_base_url,'--installation-id',applicationId,'--install-dir',path.join(out,'private/client'),'--source',path.join(out,'private/codex'),'--no-start'],'local-cli-install',{...env,FLOWBASE_AGENT_LOGS_API_KEY:'proof-only-key'});
  assert.match(execFileSync(path.join(out,'private/client/bin/codex-logs-collector'),['--version'],{encoding:'utf8'}),/0\.1\.0/);
  assert.equal(remote.requests.length,remoteCount,'terminal installation must not contact remote');
  // Clear process catalog caches: retained DB/package still owns offline metadata.
  await owner.dispose(); owner=null;
  await stop(api);
  api=start(binary,[],root+'/api','api-offline-restart',env);
  await ready(base+'/health');
  owner=await openTemporaryOwnerSession({apiBaseUrl:base,account:env.BOOTSTRAP_ROOT_ACCOUNT,password});
  const offlineCatalog=await request('GET','/api/console/applications/catalog');assert.equal(offlineCatalog.status,200);
  const retained=offlineCatalog.body.data.collectors.find(entry=>entry.catalog_id===installed.catalog_id);
  assert.equal(retained.installation_status,'installed');assert.equal(retained.installed_version,'0.1.0');
  const restartedRemoteCount=remote.requests.length;
  assert.equal((await fetch(base+retained.shell_installer_url)).status,200);
  assert.equal(remote.requests.length,restartedRemoteCount);
  receipt.checks.push({name:'offline catalog survives API restart and retains local download URLs',status:'pass'});
  // Signed tamper authenticity: a retained archive changed on disk loses all usable URLs.
  const inventory=await request('GET','/api/console/settings/extension-center/installed?category=runtime-extensions');assert.equal(inventory.status,200);
  const local=inventory.body.data.entries.find(entry=>entry.id===installed.extension_installation_id);assert.ok(local?.local_path);
  const localPath=fs.statSync(local.local_path).isDirectory()?path.join(local.local_path,'artifact.bin'):local.local_path;
  const fd=fs.openSync(localPath,'r+');const byte=Buffer.alloc(1);fs.readSync(fd,byte,0,1,20);byte[0]^=1;fs.writeSync(fd,byte,0,1,20);fs.closeSync(fd);
  assert.ok((await fetch(base+installed.shell_installer_url)).status>=400);
  const damaged=await request('GET','/api/console/applications/catalog');assert.equal(damaged.status,200);
  const unavailable=damaged.body.data.collectors.find(entry=>entry.catalog_id===installed.catalog_id);assert.equal(unavailable.installation_status,'missing');assert.equal(unavailable.asset_base_url,null);
  const runtimeCount=dock(['exec',container,'psql','-U','postgres','-d','collector_proof','-Atc',"SELECT COUNT(*) FROM extension_installations WHERE id = '"+installed.extension_installation_id+"' AND plugin_id IS NOT NULL"]);assert.equal(runtimeCount,'0');
  receipt.checks.push({name:'remote disabled: all local assets and native CLI installation',status:'pass',assets:assetReceipt},{name:'public selectors/version/node fail closed and archive tamper suppresses commands',status:'pass'},{name:'client package created no runtime installation',status:'pass'});
  assert.equal(resourceBreach,false);receipt.status='pass';
}catch(error){receipt.status='fail';receipt.error=error.message;process.exitCode=1;}
finally {
  if(timer)clearInterval(timer);
  try{if(owner){const cookie=owner.cookie;await owner.dispose();assert.equal((await fetch(base+'/api/console/session',{headers:{cookie}})).status,401);receipt.cleanup.session_revoked=true;}}catch(error){receipt.cleanup.session_error=error.message;process.exitCode=1;}
  await stop(child);await stop(frontend);await stop(api);
  if(remote)await remote.close();
  if(containerId)try{const info=JSON.parse(dock(['inspect',container]))[0];assert.equal(info.Id,containerId);assert.equal(info.Config.Labels['codex.qa.owner'],receipt.source_sha);dock(['rm','-fv',container]);receipt.cleanup.database_removed=true;}catch(error){receipt.cleanup.database_error=error.message;process.exitCode=1;}
  receipt.cleanup.shared_services_unchanged=protectedPids.every(item=>{try{return fs.readFileSync(`/proc/${item.pid}/stat`,'utf8').split(' ')[21]===item.start&&fs.readlinkSync(`/proc/${item.pid}/cwd`)===item.cwd;}catch{return false;}});
  receipt.cleanup.ports_free=[7801,3102,dbPort].every(port=>execFileSync('ss',['-H','-ltn','sport','=',':'+port],{encoding:'utf8'}).trim()==='');
  if(out){fs.rmSync(path.join(out,'private'),{recursive:true,force:true});fs.writeFileSync(path.join(out,'runtime.json'),JSON.stringify(receipt,null,2));}
  fs.rmSync(path.join(root,'web/app/.vite/collector-distribution-proof'),{recursive:true,force:true});
  process.stdout.write(JSON.stringify({status:receipt.status,error:receipt.error,checks:receipt.checks.map(({name,status,exit})=>({name,status,exit})),cleanup:receipt.cleanup,peaks:receipt.peaks})+'\n');
}
})();
