'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const {prove, load} = require('../source-proof');
const source = load(process.env.PRODUCT_ROOT || process.cwd());
test('sealed same-Value identity candidate passes exact source invariant', () => assert.equal(prove(source).status, 'pass'));
test('controlled rehash in client Section is rejected', () => {
 const semantic = source.semantic.replace('PreparedCanonicalRuntimeJson::new(value)?;', 'PreparedCanonicalRuntimeJson::new(value)?; PreparedCanonicalRuntimeJson::new(value)?;');
 assert.throws(() => prove({...source, semantic}), /one preparation/);
});
test('controlled writer second Value parameter is rejected', () => {
 const foundation = source.foundation.replace("prepared: PreparedCanonicalRuntimeJson<'_>,", "prepared: PreparedCanonicalRuntimeJson<'_>,\n    content: &Value,");
 assert.throws(() => prove({...source, foundation}), /another Value/);
});
test('controlled exposed digest field is rejected', () => {
 assert.throws(() => prove({...source, prepared:source.prepared.replace('    hash: String,', '    pub(super) hash: String,')}), /private/);
});
test('controlled hash-only collision acceptance is rejected', () => {
 const foundation = source.foundation.replace('stored_content != *content || stored_byte_size != byte_size', 'stored_byte_size != byte_size');
 assert.throws(() => prove({...source, foundation}), /collision/);
});
