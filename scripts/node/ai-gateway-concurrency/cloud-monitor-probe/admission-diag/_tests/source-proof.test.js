'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process');
const{BASE,structural}=require('../source-proof');
const root=process.env.ADMISSION_SOURCE_DIR||process.cwd(),dir='api/crates/control-plane/src/client_trajectory';
const original=cp.execFileSync('git',['-C',root,'show',BASE+':'+dir+'/mod.rs'],{encoding:'utf8',maxBuffer:1024*1024}),current=fs.readFileSync(path.join(root,dir,'mod.rs'),'utf8'),diagnostics=fs.readFileSync(path.join(root,dir,'diagnostics.rs'),'utf8');
test('diagnostic source keeps business owners awaits watermark batching and backpressure',()=>{assert.equal(structural({original,current,diagnostics}).status,'pass');});
test('source contract rejects removed complete await and expanded queue',()=>{for(const broken of [current.replace('self.wait_finished().await;',''),current.replace('const QUEUE_RECORDS: usize = 8;','const QUEUE_RECORDS: usize = 16;'),current.replace('while replay_cursor < receipt.persisted_through','while true')])assert.throws(()=>structural({original,current:broken,diagnostics}));});
test('source contract refuses payload logging and per-token debug instrumentation',()=>{for(const broken of [diagnostics+'\ntracing::debug!(value);',diagnostics+'\ninput.bytes.to_vec();',diagnostics.replace('OnceLock<bool>','bool')])assert.throws(()=>structural({original,current,diagnostics:broken}));});
