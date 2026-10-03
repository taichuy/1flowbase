'use strict';
const fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process'),assert=require('node:assert/strict');
const BASE='3eccf00ea1f32ec5d75eeeb3acee9cffbdd7b627',DIR='api/crates/control-plane/src/client_trajectory';
const ALLOWED=['mod.rs','diagnostics.rs','_tests/mod.rs','_tests/admission.rs','_tests/diagnostics.rs'].map(p=>DIR+'/'+p);
function structural({original,current,diagnostics}){
 assert.equal((original.match(/\.await\b/g)||[]).length,(current.match(/\.await\b/g)||[]).length,'no awaits added or removed');assert.equal((original.match(/tokio::spawn\(/g)||[]).length,(current.match(/tokio::spawn\(/g)||[]).length,'worker concurrency unchanged');
 for(const marker of ['const FRAME_BYTES: usize = 64 * 1024;','const QUEUE_RECORDS: usize = 8;','mpsc::channel(QUEUE_RECORDS)','while replay_cursor < receipt.persisted_through','receiver.close();','state.binding_closed.store(true, Ordering::Release);','replay_cursor = frame.sequence;'])assert.ok(current.includes(marker),'required semantic marker '+marker);
 assert.match(current,/\.send\(frame\)[^]*?\.await/);assert.match(current,/self\.finish\(\);\s*self\.wait_finished\(\)\.await/);assert.match(current,/let result = repository\.append\(&input\)\.await;/);assert.match(current,/let result = repository\.replay\(id, replay_cursor\)\.await;/);assert.match(current,/\.archive\(&AppendClientTrajectoryArchiveInput/);
 assert.doesNotMatch(current,/MeasuredWriter|try_send|spawn_blocking|reserve_owned|Semaphore|tokio::time::timeout/,'no queue replacement/concurrency/timeout');
 assert.match(diagnostics,/OnceLock<bool>/);assert.match(diagnostics,/std::env::var\(ENABLE_ENV\)\.as_deref\(\) == Ok\("1"\)/);assert.match(diagnostics,/"FLOWBASE_CLIENT_TRAJECTORY_DIAGNOSTICS"/);assert.match(diagnostics,/pub\(super\) const COUNT: usize = 13/);assert.equal((diagnostics.match(/tracing::info!/g)||[]).length,1);assert.doesNotMatch(diagnostics,/AppendClientTrajectoryInput|\.bytes\b|serde_json::to_value\(.*fact|tracing::debug!|tokio::spawn/,'only fixed counters and IDs can be logged');
 return{status:'pass',base:BASE,awaits:(current.match(/\.await\b/g)||[]).length,spawn_count:(current.match(/tokio::spawn\(/g)||[]).length,scope:'source invariants only; no Rust compile or runtime proof'};
}
function prove(root=process.cwd(),source=null){
 const git=args=>cp.execFileSync('git',['-C',root,...args],{encoding:'utf8',maxBuffer:1024*1024}).trim();source??=git(['rev-parse','HEAD']);assert.match(source,/^[a-f0-9]{40}$/);const paths=git(['diff','--name-only',BASE,source,'--','api']).split('\n').filter(Boolean);assert.deepEqual(paths.slice().sort(),ALLOWED.slice().sort(),'only explicit client diagnostic source/test files may differ');
 for(const subtree of ['api/apps/api-server','api/crates/storage','api/crates/interface-runtime','api/crates/control-plane-contracts','api/crates/control-plane/src/orchestration_runtime'])assert.equal(git(['rev-parse',source+':'+subtree]),git(['rev-parse',BASE+':'+subtree]),'unchanged owner '+subtree);
 const result=structural({original:git(['show',BASE+':'+DIR+'/mod.rs']),current:git(['show',source+':'+DIR+'/mod.rs']),diagnostics:git(['show',source+':'+DIR+'/diagnostics.rs'])});return{...result,product_source:source,api_tree:git(['rev-parse',source+':api']),paths};
}
if(require.main===module)console.log(JSON.stringify(prove(process.argv[2]||process.cwd()),null,2));module.exports={BASE,ALLOWED,structural,prove};
