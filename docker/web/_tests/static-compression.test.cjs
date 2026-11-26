const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');
const { spawn, spawnSync } = require('node:child_process');
const { setTimeout: delay } = require('node:timers/promises');
const { gzipSync, gunzipSync } = require('node:zlib');

const config = fs.readFileSync(path.resolve(__dirname, '../nginx.conf'), 'utf8');
const uncommented = config.replace(/#.*$/gm, '');
const server = uncommented.slice(uncommented.indexOf('server {'));
const serverDirectives = server.slice(0, server.indexOf('location '));

function location(header) {
  const start = uncommented.indexOf(header);
  assert.notEqual(start, -1, header);
  return uncommented.slice(start, uncommented.indexOf('\n  }', start));
}

test('server compression is negotiated, inherited, thresholded and does not assume a Brotli module', () => {
  for (const directive of [
    'sendfile on;', 'tcp_nopush on;', 'tcp_nodelay on;',
    'open_file_cache max=4096 inactive=60s;',
    'gzip on;', 'gzip_static on;', 'gzip_vary on;', 'gzip_proxied any;',
    'gzip_min_length 512;', 'gzip_comp_level 5;',
  ]) assert.ok(serverDirectives.includes(directive), directive);
  const types = serverDirectives.match(/gzip_types\s+([^;]+);/)[1].split(/\s+/);
  assert.deepEqual(types, [
    'text/plain', 'text/css', 'application/javascript', 'text/javascript',
    'application/json', 'image/svg+xml', 'application/wasm',
  ]);
  assert.doesNotMatch(uncommented, /\bbrotli(?:_\w+)?\s|gzip_static\s+always/);
});

test('sign-in selection and private caching remain independent of content encoding', () => {
  const selection = uncommented.match(/map "\$args:\$http_cookie:\$http_authorization" \$sign_in_document \{([^}]+)\}/);
  assert.ok(selection);
  assert.equal(selection[1].trim().replace(/\s+/g, ' '), '"::" /sign-in-preload.html; default /index.html;');
  for (const header of ['location = / {', 'location = /sign-in {']) {
    const entry = location(header);
    assert.match(entry, /try_files \$sign_in_document \/index\.html;/);
    assert.match(entry, /add_header Cache-Control "private, no-store" always;/);
    assert.match(entry, /add_header Vary "Cookie, Authorization" always;/);
    assert.doesNotMatch(entry, /immutable|rewrite|\$http_accept_encoding/);
  }
  assert.match(location('location = /index.html {'), /add_header Cache-Control "no-cache";/);
});

test('only content-addressed assets receive immutable cache control, never their misses', () => {
  const vite = location('location ~ "^/assets/[^/]+-');
  const expression = new RegExp(vite.match(/location ~ "([^"]+)"/)[1]);
  for (const extension of ['js', 'css', 'html', 'svg', 'json', 'woff', 'woff2', 'wasm']) {
    assert.ok(expression.test(`/assets/file-abcd1234.${extension}`), extension);
    assert.equal(expression.test(`/assets/file.${extension}`), false, extension);
    assert.equal(expression.test(`/assets/file-abcd123.${extension}`), false, extension);
  }
  const native = location('location ~ "^/assets/native-declarations/');
  const nativeExpression = new RegExp(native.match(/location ~ "([^"]+)"/)[1]);
  assert.ok(nativeExpression.test(`/assets/native-declarations/${'a'.repeat(64)}.json`));
  assert.equal(nativeExpression.test(`/assets/native-declarations/${'a'.repeat(63)}.json`), false);
  for (const block of [vite, native]) {
    assert.match(block, /try_files \$uri =404;/);
    assert.match(block, /add_header Cache-Control "public, max-age=31536000, immutable";/);
    assert.doesNotMatch(block, /index\.html|immutable" always/);
  }
  const icon = location('location = /icon.svg {');
  assert.match(icon, /try_files \$uri =404;/);
  assert.match(icon, /gzip_static on;/);
  assert.match(icon, /gzip_vary on;/);
  assert.match(icon, /add_header Cache-Control "public, max-age=86400";/);
  assert.doesNotMatch(icon, /immutable|index\.html|always/);
});

function listen(server) {
  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => resolve(server.address().port));
  });
}

function request(port, url, headers = {}) {
  return new Promise((resolve, reject) => {
    const req = http.get({ host: '127.0.0.1', port, path: url, headers, agent: false }, (res) => {
      const chunks = [];
      res.on('data', (chunk) => chunks.push(chunk));
      res.on('error', reject);
      res.on('end', () => resolve({ status: res.statusCode, headers: res.headers, body: Buffer.concat(chunks) }));
    });
    req.setTimeout(2000, () => req.destroy(new Error('Nginx fixture request timed out')));
    req.on('error', reject);
  });
}

function vary(response) {
  return new Set(String(response.headers.vary || '').toLowerCase().split(/\s*,\s*/));
}

// No dependency installation, image pull or container operations. A native nginx
// is required for protocol evidence; a skip is explicitly NOT hosting validation.
const nginx = process.env.ONEFLOWBASE_NGINX_BINARY || 'nginx';
const available = spawnSync(nginx, ['-v'], { encoding: 'utf8', timeout: 3000 });
test('native nginx serves the actual repository configuration safely', {
  skip: available.error?.code === 'ENOENT' ? 'Native nginx unavailable; Root must verify hosting semantics' : false,
  timeout: 20000,
}, async (t) => {
  assert.equal(available.status, 0, available.error?.message || available.stderr);
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'flowbase-nginx-static-'));
  const upstream = http.createServer((req, res) => {
    res.setHeader('Content-Type', 'application/json');
    res.end(JSON.stringify({ url: req.url, host: req.headers.host, scheme: req.headers['x-forwarded-proto'] }));
  });
  let child;
  let spawnError;
  let exited;
  t.after(async () => {
    try {
      if (child && child.exitCode === null && child.signalCode === null && !spawnError) {
        child.kill('SIGTERM');
        const timer = setTimeout(() => child.kill('SIGKILL'), 2000);
        try { await exited; } finally { clearTimeout(timer); }
      }
    } finally {
      if (upstream.listening) await new Promise((resolve) => upstream.close(resolve));
      fs.rmSync(root, { recursive: true, force: true });
    }
  });
  const apiPort = await listen(upstream);
  const reservation = http.createServer();
  const port = await listen(reservation);
  await new Promise((resolve) => reservation.close(resolve));
  const htmlRoot = path.join(root, 'html');
  const put = (name, bytes, precompress = true) => {
    const file = path.join(htmlRoot, name);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, bytes);
    fs.utimesSync(file, 1700000000, 1700000000);
    if (precompress) {
      fs.writeFileSync(`${file}.gz`, gzipSync(bytes, { level: 9 }));
      fs.utimesSync(`${file}.gz`, 1700000000, 1700000000);
    }
  };
  const guarded = Buffer.from(`<html>guarded ${'index'.repeat(150)}</html>`);
  const anonymous = Buffer.from(`<html>anonymous ${'motion'.repeat(150)}</html>`);
  const payload = Buffer.from('compressible data '.repeat(100));
  put('index.html', guarded);
  put('sign-in-preload.html', anonymous);
  put('icon.svg', payload);
  put('small.txt', Buffer.alloc(511, 0x61), false);
  put('boundary.txt', Buffer.alloc(512, 0x61), false);
  for (const extension of ['txt', 'css', 'js', 'mjs', 'json', 'svg', 'wasm']) put(`dynamic.${extension}`, payload, false);
  for (const extension of ['js', 'css', 'html', 'svg', 'json', 'ttf', 'wasm']) put(`assets/file-abcd1234.${extension}`, payload);
  const nativePath = `/assets/native-declarations/${'a'.repeat(64)}.json`;
  put(nativePath.slice(1), payload);
  put('assets/unhashed.js', payload);
  const active = config
    .replace('listen 80;', `listen 127.0.0.1:${port};`)
    .replace('root /usr/share/nginx/html;', `root "${htmlRoot}";`)
    .replaceAll('http://api:7800', `http://127.0.0.1:${apiPort}`);
  const configuration = path.join(root, 'nginx.conf');
  fs.writeFileSync(configuration, `
worker_processes 1;
pid "${root}/nginx.pid";
error_log stderr warn;
events { worker_connections 64; }
http {
  access_log off;
  client_body_temp_path "${root}/client";
  proxy_temp_path "${root}/proxy";
  types {
    text/html html;
    text/plain txt;
    text/css css;
    application/javascript js;
    text/javascript mjs;
    application/json json;
    image/svg+xml svg;
    application/wasm wasm;
  }
  ${active}
}
`);
  child = spawn(nginx, ['-p', `${root}/`, '-c', configuration, '-g', 'daemon off; master_process off;'], {
    stdio: ['ignore', 'ignore', 'pipe'],
  });
  let stderr = '';
  child.stderr.on('data', (chunk) => { stderr = (stderr + chunk).slice(-4000); });
  child.on('error', (error) => { spawnError = error; });
  exited = new Promise((resolve) => child.once('exit', resolve));
  let ready = false;
  for (let attempt = 0; attempt < 75; attempt++) {
    if (spawnError) throw spawnError;
    assert.equal(child.exitCode, null, stderr);
    try { await request(port, '/index.html'); ready = true; break; } catch { await delay(40); }
  }
  assert.ok(ready, `Nginx fixture did not become ready: ${stderr}`);

  await t.test('static gzip honors Accept-Encoding including q=0; Brotli is not assumed', async () => {
    let modified;
    for (const encoding of ['gzip', 'gzip;q=0', 'identity', 'br']) {
      const response = await request(port, '/assets/file-abcd1234.js', { 'Accept-Encoding': encoding, Via: '1.1 fixture-proxy' });
      assert.equal(response.status, 200);
      assert.match(response.headers['cache-control'], /immutable/);
      assert.ok(vary(response).has('accept-encoding'));
      if (encoding === 'gzip') {
        assert.equal(response.headers['content-encoding'], 'gzip');
        assert.deepEqual(response.body, gzipSync(payload, { level: 9 }));
        assert.deepEqual(gunzipSync(response.body), payload);
        modified = response.headers['last-modified'];
      } else {
        assert.equal(response.headers['content-encoding'], undefined);
        assert.deepEqual(response.body, payload);
        assert.equal(response.headers['last-modified'], modified);
      }
    }
  });

  await t.test('server-level dynamic gzip uses the byte threshold and all required MIME types', async () => {
    const small = await request(port, '/small.txt', { 'Accept-Encoding': 'gzip', Via: '1.1 fixture-proxy' });
    assert.equal(small.headers['content-encoding'], undefined);
    assert.equal(small.body.length, 511);
    const boundary = await request(port, '/boundary.txt', { 'Accept-Encoding': 'gzip', Via: '1.1 fixture-proxy' });
    assert.equal(boundary.headers['content-encoding'], 'gzip');
    assert.deepEqual(gunzipSync(boundary.body), Buffer.alloc(512, 0x61));
    for (const extension of ['txt', 'css', 'js', 'mjs', 'json', 'svg', 'wasm']) {
      const response = await request(port, `/dynamic.${extension}`, { 'Accept-Encoding': 'gzip', Via: '1.1 fixture-proxy' });
      assert.equal(response.headers['content-encoding'], 'gzip', extension);
      assert.deepEqual(gunzipSync(response.body), payload);
    }
  });

  await t.test('every credential/query branch keeps guarded bytes and private-no-store with gzip or identity', async () => {
    const cases = [
      ['/sign-in', {}, anonymous],
      ['/sign-in?login_entry_id=entry', {}, guarded],
      ['/sign-in?auth_error=denied', {}, guarded],
      ['/sign-in?locale=en', {}, guarded],
      ['/sign-in?request_id=mcp', {}, guarded],
      ['/sign-in?unknown=value', {}, guarded],
      ['/sign-in', { Cookie: 'session=present' }, guarded],
      ['/sign-in', { Authorization: 'Bearer present' }, guarded],
      ['/sign-in?unknown=value', { Cookie: 'session=present', Authorization: 'Bearer present' }, guarded],
    ];
    for (const encoding of ['gzip', 'identity']) {
      for (const [url, headers, expected] of cases) {
        const response = await request(port, url, { ...headers, 'Accept-Encoding': encoding });
        assert.equal(response.status, 200, url);
        assert.equal(response.headers['cache-control'], 'private, no-store', url);
        for (const field of ['cookie', 'authorization', 'accept-encoding']) assert.ok(vary(response).has(field), field);
        assert.equal(response.headers['content-encoding'], encoding === 'gzip' ? 'gzip' : undefined);
        assert.deepEqual(encoding === 'gzip' ? gunzipSync(response.body) : response.body, expected, url);
      }
    }
  });

  await t.test('hashed assets/native JSON cache hits only; unhashed assets and misses are never immutable', async () => {
    for (const url of [nativePath, ...['js', 'css', 'html', 'svg', 'json', 'ttf', 'wasm'].map((ext) => `/assets/file-abcd1234.${ext}`)]) {
      const response = await request(port, url, { 'Accept-Encoding': 'gzip', Via: '1.1 fixture-proxy' });
      assert.equal(response.status, 200, url);
      assert.match(response.headers['cache-control'], /immutable/);
      assert.deepEqual(gunzipSync(response.body), payload);
    }
    for (const url of ['/assets/missing-abcd1234.json', `/assets/native-declarations/${'b'.repeat(64)}.json`]) {
      const response = await request(port, url);
      assert.equal(response.status, 404, url);
      assert.doesNotMatch(response.headers['cache-control'] || '', /immutable/);
    }
    const unhashed = await request(port, '/assets/unhashed.js');
    assert.equal(unhashed.status, 200);
    assert.doesNotMatch(unhashed.headers['cache-control'] || '', /immutable/);
    const icon = await request(port, '/icon.svg', { 'Accept-Encoding': 'gzip', Via: '1.1 fixture-proxy' });
    assert.equal(icon.status, 200);
    assert.equal(icon.headers['cache-control'], 'public, max-age=86400');
    assert.doesNotMatch(icon.headers['cache-control'], /immutable/);
    assert.equal(icon.headers['content-encoding'], 'gzip');
    assert.ok(vary(icon).has('accept-encoding'));
    assert.deepEqual(gunzipSync(icon.body), payload);
    fs.rmSync(path.join(htmlRoot, 'icon.svg'));
    fs.rmSync(path.join(htmlRoot, 'icon.svg.gz'));
    const missingIcon = await request(port, '/icon.svg');
    assert.equal(missingIcon.status, 404);
    assert.equal(missingIcon.headers['cache-control'], undefined);
  });

  await t.test('API/v1 proxy paths and forwarded origin still reach the same upstream', async () => {
    for (const prefix of ['/api/', '/v1/']) {
      const url = `${prefix}fixture?query=preserved`;
      const response = await request(port, url, { 'X-Forwarded-Proto': 'https' });
      assert.equal(response.status, 200);
      assert.deepEqual(JSON.parse(response.body), { url, host: `127.0.0.1:${port}`, scheme: 'https' });
    }
  });
});
