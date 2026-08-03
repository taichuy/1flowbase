const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
const composeFiles = [
  'docker/docker-compose.yaml',
  'docker/docker-compose.dev.yaml',
  'docker/docker-compose.middleware.yaml',
];

test('AC-002: bundled PostgreSQL services default to version 18 and its versioned data root', () => {
  for (const relativePath of composeFiles) {
    const compose = fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');
    assert.match(compose, /image: postgres:18-alpine/u, relativePath);
    assert.match(compose, /:\/var\/lib\/postgresql(?:\r?\n|$)/u, relativePath);
    assert.match(compose, /1flowbase-postgres-entrypoint\.sh/u, relativePath);
    assert.doesNotMatch(compose, /:\/var\/lib\/postgresql\/data(?:\r?\n|$)/u, relativePath);
  }
});

test('AC-003: PostgreSQL entrypoint rejects a pre-18 root data layout before initialization', () => {
  const entrypoint = fs.readFileSync(
    path.join(repoRoot, 'docker/postgres/entrypoint.sh'),
    'utf8'
  );

  assert.match(entrypoint, /\/var\/lib\/postgresql\/PG_VERSION/u);
  assert.match(entrypoint, /major upgrade is required/u);
  assert.match(entrypoint, /exec \/usr\/local\/bin\/docker-entrypoint\.sh/u);
});
