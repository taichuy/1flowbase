'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),{checkPacket}=require('../source-proof'),p=require('../freeze.json');
test('fresh updated-dev pair cannot reuse old binaries',()=>{checkPacket(p);assert.throws(()=>checkPacket({...p,old_binary_reuse:true}));});
test('finite same provider and levels reject expansion',()=>{assert.throws(()=>checkPacket({...p,levels:[1,8,64]}));assert.throws(()=>checkPacket({...p,provider_sha:'stale'}));});
