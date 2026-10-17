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
