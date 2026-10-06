const test = require('node:test');
const assert = require('node:assert/strict');
const { scanRustSource } = require('../core.js');
const scan = content => scanRustSource({ relativePath: 'api/crates/domain/src/response.rs', content });

test('sensitive logging is independent of macro layout and delimiter', () => {
  for (const content of [
    'tracing::info!(\n token = %token,\n "received"\n);',
    'tracing::error! { "failed: {}", secret }',
    'println!["{}", api_key];',
  ]) assert.deepEqual(scan(content).map(item => item.rule), ['no-sensitive-logging'], content);
  assert.equal(scan('tracing::info!(\n token = %token,\n "received"\n);')[0].line, 2);
});

test('sensitive fields are detected in multiline derives and inline structs', () => {
  for (const content of [
    '#[derive(\n Debug, serde::Serialize,\n)]\npub struct Response { pub password_hash: String }',
    '#[derive(Serialize)] pub struct Response { pub token_hash: String }',
    '#[derive(Serialize)]\n#[serde(rename_all = "camelCase")]\npub(crate) struct Response<T> { pub secret_value: T }',
    '#[derive(Serialize)]\nstruct Response<T: Fn() -> String, const N: usize = { 2 }> { api_key_secret: [T; N] }',
  ]) assert.deepEqual(scan(content).map(item => item.rule), ['no-sensitive-serialize'], content);
});

test('static messages and nonserializable records do not leak sensitive values', () => {
  assert.deepEqual(scan([
    'tracing::info!("password token secret api_key", count = 1);',
    'tracing::info!("{}", { let label = "password"; label });',
    '#[derive(Debug)] struct Internal { password_hash: String }',
    '#[derive(Serialize)] enum Status { Ready }',
    'struct InternalToken { token_hash: String }',
    '#[derive(Serialize)] struct Public { id: String }',
    'fn hash() { let password_hash = compute(); }',
  ].join('\n')), []);
});

test('unconditional serde skip is legal but conditional skip cannot exempt credentials', () => {
  for (const skip of ['skip', 'skip_serializing']) {
    assert.deepEqual(scan(`#[derive(Serialize)] struct Response {
      #[serde(${skip})] pub password_hash: String,
      #[serde(rename = "hash", ${skip})] token_hash: String,
    }`), []);
  }
  for (const attr of ['skip_deserializing', 'skip_serializing_if = "String::is_empty"', 'rename = "skip_serializing"']) {
    assert.equal(scan(`#[derive(Serialize)] struct Response { #[serde(${attr})] token_hash: String }`).length, 1, attr);
  }
  assert.equal(scan('#[derive(Serialize)] struct Response { #[serde(skip)] id: String, token_hash: String }').length, 1);
});

test('test-only sensitive records are exempt without hiding adjacent production records', () => {
  const findings = scan('#[cfg(test)] #[derive(Serialize)] struct Fixture { token_hash: String }\n#[derive(Serialize)] struct Response { token_hash: String }');
  assert.deepEqual(findings.map(item => [item.rule, item.line]), [['no-sensitive-serialize', 2]]);
});


test('raw identifiers and spaced visibility preserve unconditional serde skip', () => {
  for (const field of ['r#token_hash', 'pub (crate) token_hash', 'pub(in crate::auth) r#token_hash']) {
    for (const skip of ['skip', 'skip_serializing']) {
      assert.deepEqual(scan(`#[derive(Serialize)] struct Response { # [serde(${skip})] ${field}: String }`), []);
    }
    assert.deepEqual(scan(`#[derive(Serialize)] struct Response { ${field}: String }`).map(item => item.rule), ['no-sensitive-serialize']);
  }
});

test('field type const bindings are not serialized fields and generic commas do not hide later fields', () => {
  assert.deepEqual(scan(`#[derive(Serialize)] struct Public {
    bytes: [u8; { let token_hash: usize = 1; token_hash }],
    pair: Result<String, Vec<(u8, u8)>>,
  }`), []);
  assert.deepEqual(scan(`#[derive(Serialize)] struct Response {
    pair: Result<String, Vec<(u8, u8)>>,
    token_hash: String,
  }`).map(item => item.rule), ['no-sensitive-serialize']);
});

test('where function bounds and spaced attributes retain the named struct owner', () => {
  assert.deepEqual(scan(`#[derive(Serialize)]
    # [serde(rename_all = "camelCase")]
    struct Response<T> where T: Fn() -> String { password_hash: String, marker: T }
  `).map(item => item.rule), ['no-sensitive-serialize']);
});
