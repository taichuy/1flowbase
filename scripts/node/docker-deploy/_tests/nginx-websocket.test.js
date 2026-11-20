const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const config = fs.readFileSync(
  path.resolve(__dirname, '../../../../docker/web/nginx.conf'),
  'utf8',
);

for (const prefix of ['/api/', '/v1/']) {
  test(`Docker proxy preserves WebSocket upgrades under ${prefix}`, () => {
    const start = config.indexOf(`location ${prefix} {`);
    assert.notEqual(start, -1);
    const location = config.slice(start, config.indexOf('}', start));
    assert.match(location, /proxy_http_version\s+1\.1;/);
    assert.match(location, /proxy_set_header\s+Upgrade\s+\$http_upgrade;/);
    assert.match(location, /proxy_set_header\s+Connection\s+\$connection_upgrade;/);
  });
}


test('hashed declaration JSON is cached as an immutable file, never SPA fallback or a cached miss', () => {
  const start = config.indexOf('location ~ "^/assets/native-declarations/');
  assert.notEqual(start, -1);
  // The regex contains {64}; find the block delimiter rather than that brace.
  const end = config.indexOf('\n  }', start);
  const location = config.slice(start, end);
  assert.match(location, /try_files \$uri =404;/);
  assert.match(location, /gzip on;/);
  assert.match(location, /gzip_types application\/json;/);
  assert.match(location, /gzip_vary on;/);
  assert.match(location, /add_header Cache-Control "public, max-age=31536000, immutable";/);
  assert.doesNotMatch(location, /index\.html|immutable" always/);
});
