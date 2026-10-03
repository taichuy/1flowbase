'use strict';
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const BASE = 'api/crates/storage/durable/postgres/src/orchestration_runtime_repository';
function functionSource(source, name) {
  const start = source.indexOf('fn ' + name + '(');
  assert.notEqual(start, -1, 'missing ' + name);
  const end = source.indexOf('\n}\n', start);
  assert.notEqual(end, -1, 'unterminated ' + name);
  return source.slice(start, end + 3);
}
function prove({prepared, foundation, semantic}) {
  const structure = prepared.match(/pub\(super\) struct PreparedCanonicalRuntimeJson[^]*?\n\}/)?.[0];
  assert.ok(structure, 'sealed prepared type required');
  assert.match(structure, /value: &'a Value/);
  assert.match(structure, /hash: String/);
  assert.match(structure, /byte_size: i64/);
  assert.doesNotMatch(structure.replace('pub(super) struct', 'struct'), /\bpub\b/, 'fields must be private');
  const writer = functionSource(foundation, 'put_prepared_canonical_runtime_content_with_creation');
  const header = writer.slice(0, writer.indexOf('{'));
  assert.match(header, /prepared: PreparedCanonicalRuntimeJson<'_>/);
  assert.doesNotMatch(header, /content:|hash:|byte_size:|&Value/, 'writer cannot accept another Value or digest');
  assert.match(writer, /let content = prepared\.value\(\);/);
  assert.match(writer, /let content_hash = prepared\.hash\(\);/);
  assert.match(writer, /let byte_size = prepared\.byte_size\(\);/);
  assert.doesNotMatch(writer, /canonical_runtime_json_identity|PreparedCanonicalRuntimeJson::new/);
  assert.match(writer, /stored_content != \*content \|\| stored_byte_size != byte_size/, 'full collision check required');
  assert.match(writer, /where scope_id = \$1 and application_id = \$2 and content_hash = \$3/);
  const section = functionSource(semantic, 'write_section');
  assert.equal((section.match(/PreparedCanonicalRuntimeJson::new\(value\)/g) || []).length, 1, 'one preparation per section');
  assert.match(section, /put_prepared_canonical_runtime_content_with_creation/);
  assert.doesNotMatch(section, /value_identity\(value\)|put_canonical_runtime_content_with_creation\(/);
  assert.match(section, /digest_bytes\(prepared\.hash\(\)\)/);
  return {status:'pass', boundary:'source/type and exact caller invariant only; not Rust compile/behavior or CPU evidence'};
}
function load(root) {
 const read = file => fs.readFileSync(path.join(root, BASE, file), 'utf8');
 return {prepared:read('canonical_runtime_json.rs'),foundation:read('storage_foundation_methods.rs'),semantic:read('client_trajectory/semantic.rs')};
}
if (require.main === module) console.log(JSON.stringify(prove(load(process.argv[2] || process.cwd()))));
module.exports = {prove, load, functionSource};
