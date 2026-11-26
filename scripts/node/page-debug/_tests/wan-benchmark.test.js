'use strict';

const assert = require('node:assert/strict');
const http = require('node:http');
const http2 = require('node:http2');
const net = require('node:net');
const test = require('node:test');

const { startH2WanProxy } = require('../wan-benchmark/h2-wan-proxy.js');
const {
  createPgFrontendFrameTap,
  rewriteDatabaseUrlForProxy,
  startPgDelayProxy,
} = require('../wan-benchmark/pg-delay-proxy.js');

function buildPgStartupFrame() {
  const payload = Buffer.from('user\0postgres\0\0', 'ascii');
  const len = 8 + payload.length;
  const header = Buffer.alloc(8);
  header.writeInt32BE(len, 0);
  header.writeInt32BE(196608, 4); // v3.0
  return Buffer.concat([header, payload]);
}

function buildPgMessage(typeChar, bodyBytes = Buffer.alloc(0)) {
  const out = Buffer.alloc(5 + bodyBytes.length);
  out[0] = typeChar.charCodeAt(0);
  out.writeInt32BE(4 + bodyBytes.length, 1);
  bodyBytes.copy(out, 5);
  return out;
}

test('pg-delay-proxy counts Simple Query and Extended Query Execute/Sync frames across split TCP chunks', async () => {
  let executions = 0;
  let roundTrips = 0;
  const tap = createPgFrontendFrameTap((delta) => {
    executions += delta.executions;
    roundTrips += delta.roundTrips;
  });

  const sslReq = Buffer.alloc(8);
  sslReq.writeInt32BE(8, 0);
  sslReq.writeInt32BE(80877103, 4);

  const wire = Buffer.concat([
    sslReq,
    buildPgStartupFrame(),
    buildPgMessage('Q', Buffer.from('SELECT 1\0')),
    buildPgMessage('P', Buffer.from('stmt1\0SELECT $1\0\0\0')),
    buildPgMessage('B', Buffer.from('\0stmt1\0\0\0\0\0')),
    buildPgMessage('E', Buffer.from('\0\0\0\0\0')),
    buildPgMessage('E', Buffer.from('\0\0\0\0\0')),
    buildPgMessage('S'),
  ]);

  // Feed 3 bytes at a time to prove chunk-splitting resilience
  for (let i = 0; i < wire.length; i += 3) {
    tap(wire.subarray(i, i + 3));
  }

  assert.equal(executions, 3); // 1 'Q' + 2 'E'
  assert.equal(roundTrips, 2); // 1 'Q' + 1 'S'

  const rewritten = rewriteDatabaseUrlForProxy(
    'postgres://taichuy:secret@localhost:5432/oneflowbase?sslmode=prefer',
    15432,
  );
  assert.equal(rewritten.upstreamHost, 'localhost');
  assert.equal(rewritten.upstreamPort, 5432);
  assert.match(rewritten.proxiedDatabaseUrl, /127\.0\.0\.1:15432/);
  assert.match(rewritten.proxiedDatabaseUrl, /sslmode=disable/);
});

test('pg-delay-proxy forwards bytes losslessly and enforces RTT delay', async () => {
  const upstream = net.createServer((sock) => {
    sock.on('data', (chunk) => sock.write(chunk));
  });
  await new Promise((r) => upstream.listen(0, '127.0.0.1', r));
  const upstreamPort = upstream.address().port;

  const proxy = await startPgDelayProxy({
    upstreamHost: '127.0.0.1',
    upstreamPort,
    rttMs: 20,
  });

  try {
    const client = net.connect(proxy.port, '127.0.0.1');
    await new Promise((r) => client.once('connect', r));
    const frame = Buffer.concat([
      buildPgStartupFrame(),
      buildPgMessage('Q', Buffer.from('SELECT 42\0')),
    ]);
    const t0 = performance.now();
    const echoed = await new Promise((resolve) => {
      const chunks = [];
      let received = 0;
      client.on('data', (c) => {
        chunks.push(c);
        received += c.length;
        if (received >= frame.length) resolve(Buffer.concat(chunks));
      });
      client.write(frame);
    });
    const elapsed = performance.now() - t0;
    assert.deepEqual(echoed, frame);
    assert.ok(elapsed >= 15, `expected >= 15ms RTT, got ${elapsed}ms`);
    assert.equal(proxy.getStats().sqlExecutions, 1);
    assert.equal(proxy.getStats().sqlRoundTrips, 1);
    client.destroy();
  } finally {
    await proxy.close();
    await new Promise((r) => upstream.close(r));
  }
});

test('h2-wan-proxy multiplexes over single TLS H2 session and reproduces small-request queueing behind large stream', async () => {
  const upstream = http.createServer((req, res) => {
    if (req.url === '/api/heavy') {
      res.writeHead(200, { 'content-type': 'application/octet-stream' });
      res.end(Buffer.alloc(48_000, 0x61)); // 48 KB
      return;
    }
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end(JSON.stringify({ ok: true })); // 11 B
  });
  await new Promise((r) => upstream.listen(0, '127.0.0.1', r));
  const upstreamOrigin = `http://127.0.0.1:${upstream.address().port}`;

  const proxy = await startH2WanProxy({
    apiUpstreamOrigin: upstreamOrigin,
    downloadBytesPerSec: 100_000, // 100 KB/s
    burstBytes: 8_192,
    rttMs: 20,
  });

  const h2Client = http2.connect(proxy.origin, { rejectUnauthorized: false });
  try {
    const fetchStream = (urlPath) =>
      new Promise((resolve, reject) => {
        const req = h2Client.request({ ':path': urlPath, ':method': 'GET' });
        const chunks = [];
        let status = 0;
        req.on('response', (headers) => {
          status = headers[':status'];
        });
        req.on('data', (c) => chunks.push(c));
        req.on('end', () => resolve({ status, bytes: Buffer.concat(chunks).length }));
        req.on('error', reject);
        req.end();
      });

    const [heavy, tiny] = await Promise.all([
      fetchStream('/api/heavy'),
      fetchStream('/api/tiny'),
    ]);
    assert.equal(heavy.status, 200);
    assert.equal(heavy.bytes, 48_000);
    assert.equal(tiny.status, 200);

    const stats = proxy.getStats();
    assert.equal(stats.requests.length, 2);
    const heavyRec = stats.requests.find((r) => r.pathname === '/api/heavy');
    assert.ok(heavyRec.totalMs >= 350, `expected shaped heavy transfer >= 350ms, got ${heavyRec.totalMs}ms`);
  } finally {
    h2Client.close();
    await proxy.close();
    await new Promise((r) => upstream.close(r));
  }
});
