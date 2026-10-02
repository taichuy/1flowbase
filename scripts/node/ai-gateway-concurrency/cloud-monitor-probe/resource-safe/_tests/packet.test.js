'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),{checkPacket}=require('../source-proof'),{distinctRuns}=require('../../lock-scope/scope-contract'),p=require('../freeze.json');
test('fresh safe-head pair cannot reuse old binaries',()=>{checkPacket(p);assert.throws(()=>checkPacket({...p,old_binary_reuse:true}));});
test('finite provider and levels reject expansion',()=>{assert.throws(()=>checkPacket({...p,levels:[1,8,64]}));assert.throws(()=>checkPacket({...p,provider_sha:'stale'}));});
test('C1 and C8 require exact distinct successful runs',()=>{distinctRuns([{ok:true,response_id:'one'}],1);assert.throws(()=>distinctRuns([{ok:true,response_id:'one'}],8));assert.throws(()=>distinctRuns([{ok:true,response_id:'one'}],64));});
