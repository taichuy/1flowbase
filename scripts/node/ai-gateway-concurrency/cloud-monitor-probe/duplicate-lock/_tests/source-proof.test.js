'use strict';
const test=require('node:test'),assert=require('node:assert/strict'),{checkFiles,FILES}=require('../source-proof');
test('Exact three-file candidate proof accepts frozen8ec and rejects extra production/7f paths and changed bytes',()=>{const names=Object.keys(FILES);checkFiles(names,FILES);assert.throws(()=>checkFiles([...names,'api/crates/control-plane/src/orchestration_runtime/provider_invoker.rs'],FILES));assert.throws(()=>checkFiles(names,{...FILES,[names[2]]:'changed'}));assert.throws(()=>checkFiles([],{}));});
