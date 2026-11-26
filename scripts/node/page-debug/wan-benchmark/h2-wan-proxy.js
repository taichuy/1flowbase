'use strict';

const fs = require('node:fs');
const http = require('node:http');
const http2 = require('node:http2');
const path = require('node:path');
const { generateLoopbackTlsCredentials } = require('./tls-self-signed.js');

const MIME_TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'application/javascript; charset=utf-8',
  '.mjs': 'application/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.wasm': 'application/wasm',
};

function isViteHashedAsset(relativePath) {
  if (!relativePath.startsWith('assets/')) return false;
  const fileName = relativePath.slice('assets/'.length);
  if (fileName.includes('/')) return false;
  const dotIdx = fileName.lastIndexOf('.');
  if (dotIdx <= 9) return false;
  const stem = fileName.slice(0, dotIdx);
  if (stem[stem.length - 9] !== '-') return false;
  return /^[A-Za-z0-9_-]{8}$/.test(stem.slice(-8));
}

function classifyStaticCacheControl(relativePath) {
  if (relativePath.endsWith('.html')) {
    return isViteHashedAsset(relativePath)
      ? 'public, max-age=31536000, immutable'
      : 'no-cache';
  }
  if (
    relativePath.startsWith('external-npm/assets/') ||
    /^assets\/native-declarations\/[0-9a-f]{64}\.json$/i.test(relativePath) ||
    isViteHashedAsset(relativePath)
  ) {
    return 'public, max-age=31536000, immutable';
  }
  if (relativePath === 'icon.svg') return 'public, max-age=86400';
  if (
    relativePath === 'external-npm/manifest.json' ||
    relativePath === 'authenticated-prewarm.json' ||
    relativePath === 'scenario-chunk-manifest.json'
  ) {
    return 'no-cache';
  }
  return 'public, max-age=0, must-revalidate';
}

/**
 * Shared round-robin HTTP/2 server-egress token bucket.
 * Simulates a single cloud server NIC (e.g. 150 KB/s = 1.2 Mbps) where concurrent H2 streams
 * interleave DATA frames (4 KiB slices) in the same outbound send buffer.
 */
function createSharedEgressShaper({
  rateBytesPerSec = 150_000,
  burstBytes = 16_384,
  frameBytes = 4_096,
} = {}) {
  let tokens = burstBytes;
  let lastRefillMs = performance.now();
  let totalWireBytes = 0;
  const activeJobs = [];

  const timer = setInterval(() => {
    const now = performance.now();
    const elapsedSec = Math.max(0, (now - lastRefillMs) / 1000);
    lastRefillMs = now;
    tokens = Math.min(burstBytes, tokens + elapsedSec * rateBytesPerSec);

    let visits = activeJobs.length;
    while (activeJobs.length > 0 && visits > 0 && tokens >= 1) {
      visits -= 1;
      const job = activeJobs.shift();
      if (job.isAborted()) {
        job.resolve();
        continue;
      }
      if (job.readyAt > now) {
        activeJobs.push(job);
        continue;
      }
      if (job.body.length === 0) {
        job.onFirstByte(now);
        job.finish(now);
        job.resolve();
        continue;
      }
      const remaining = job.body.length - job.offset;
      const grant = Math.min(Math.floor(tokens), frameBytes, remaining);
      if (grant <= 0) {
        activeJobs.unshift(job);
        break;
      }
      if (job.offset === 0) {
        job.onFirstByte(now);
      }
      const slice = job.body.subarray(job.offset, job.offset + grant);
      job.offset += grant;
      tokens -= grant;
      totalWireBytes += grant;
      job.writeSlice(slice);

      if (job.offset >= job.body.length) {
        job.finish(now);
        job.resolve();
      } else {
        activeJobs.push(job);
      }
    }
  }, 5);

  return {
    get totalWireBytes() {
      return totalWireBytes;
    },
    resetCounter() {
      totalWireBytes = 0;
      tokens = burstBytes;
      lastRefillMs = performance.now();
    },
    enqueue({ body, readyAt, isAborted, onFirstByte, writeSlice, finish }) {
      return new Promise((resolve) => {
        activeJobs.push({
          body,
          offset: 0,
          readyAt,
          isAborted,
          onFirstByte,
          writeSlice,
          finish,
          resolve,
        });
      });
    },
    close() {
      clearInterval(timer);
      for (const job of activeJobs.splice(0)) {
        job.resolve();
      }
    },
  };
}

function isApiPath(pathname) {
  return (
    pathname === '/api' ||
    pathname.startsWith('/api/') ||
    pathname === '/v1' ||
    pathname.startsWith('/v1/') ||
    pathname === '/health' ||
    pathname.startsWith('/health/') ||
    pathname === '/healthz' ||
    pathname === '/readyz' ||
    pathname === '/metrics' ||
    pathname === '/openapi.json'
  );
}

function resolveStaticRepresentation(webDistRoot, reqPath, reqHeaders, enableStaticBrotli) {
  const cleanRel = decodeURIComponent(reqPath).replace(/^\/+/, '');
  if (
    cleanRel.includes('\\') ||
    cleanRel.includes(':') ||
    cleanRel.split('/').some((part) => part === '..' || part.startsWith('.'))
  ) {
    return { status: 404 };
  }

  const accept = String(reqHeaders['accept'] || '');
  const acceptEncoding = String(reqHeaders['accept-encoding'] || '');
  const hasCookieOrAuth = Boolean(reqHeaders['cookie'] || reqHeaders['authorization']);

  // Match docker/web/nginx.conf anonymous sign-in preload fast-path
  if (
    (reqPath === '/' || reqPath === '/sign-in' || reqPath === '/sign-in/') &&
    !hasCookieOrAuth
  ) {
    const preloadHtml = path.join(webDistRoot, 'sign-in-preload.html');
    if (fs.existsSync(preloadHtml) && fs.statSync(preloadHtml).isFile()) {
      return loadStaticTarget(preloadHtml, 'sign-in-preload.html', acceptEncoding, enableStaticBrotli, {
        'cache-control': 'private, no-store',
        vary: 'Accept-Encoding, Cookie, Authorization',
      });
    }
  }

  let candidate = path.join(webDistRoot, cleanRel);
  let servedRel = cleanRel;
  if (fs.existsSync(candidate) && fs.statSync(candidate).isDirectory()) {
    candidate = path.join(candidate, 'index.html');
    servedRel = cleanRel ? `${cleanRel}/index.html` : 'index.html';
  }

  if (!fs.existsSync(candidate) || !fs.statSync(candidate).isFile()) {
    const isHtmlNav = accept.includes('text/html');
    if (
      !isHtmlNav ||
      path.extname(cleanRel) !== '' ||
      cleanRel.startsWith('assets/') ||
      cleanRel.startsWith('external-npm/')
    ) {
      return { status: 404 };
    }
    candidate = path.join(webDistRoot, 'index.html');
    servedRel = 'index.html';
  }

  const resolved = fs.realpathSync(candidate);
  if (!resolved.startsWith(webDistRoot)) {
    return { status: 404 };
  }

  return loadStaticTarget(resolved, servedRel, acceptEncoding, enableStaticBrotli, {
    'cache-control': classifyStaticCacheControl(servedRel),
    vary: 'Accept-Encoding',
  });
}

function loadStaticTarget(filePath, servedRel, acceptEncoding, enableStaticBrotli, extraHeaders) {
  const ext = path.extname(filePath).toLowerCase();
  const contentType = MIME_TYPES[ext] || 'application/octet-stream';
  const stat = fs.statSync(filePath);
  const etag = `"${stat.mtimeMs.toString(16)}-${stat.size.toString(16)}"`;

  let bodyPath = filePath;
  let contentEncoding = null;
  if (enableStaticBrotli && /\bbr\b/.test(acceptEncoding) && fs.existsSync(`${filePath}.br`)) {
    bodyPath = `${filePath}.br`;
    contentEncoding = 'br';
  } else if (/\bgzip\b/.test(acceptEncoding) && fs.existsSync(`${filePath}.gz`)) {
    bodyPath = `${filePath}.gz`;
    contentEncoding = 'gzip';
  }

  const body = fs.readFileSync(bodyPath);
  const headers = {
    'content-type': contentType,
    etag,
    ...extraHeaders,
  };
  if (contentEncoding) {
    headers['content-encoding'] = contentEncoding;
  }
  headers['content-length'] = String(body.length);

  return {
    status: 200,
    headers,
    body,
    rawSize: stat.size,
  };
}

function proxyHttpRequest(upstreamOrigin, method, rawUrl, reqHeaders, reqBodyBuffer) {
  const target = new URL(rawUrl, upstreamOrigin);
  const forwardedHeaders = {};
  for (const [key, value] of Object.entries(reqHeaders)) {
    if (key.startsWith(':') || key === 'host' || key === 'connection' || key === 'keep-alive') {
      continue;
    }
    if (value !== undefined) forwardedHeaders[key] = value;
  }
  forwardedHeaders['host'] = target.host;

  return new Promise((resolve, reject) => {
    const req = http.request(
      target,
      {
        method,
        headers: forwardedHeaders,
      },
      (res) => {
        const chunks = [];
        res.on('data', (chunk) => chunks.push(chunk));
        res.on('end', () => {
          const outHeaders = {};
          for (const [k, v] of Object.entries(res.headers)) {
            if (
              k === 'connection' ||
              k === 'keep-alive' ||
              k === 'transfer-encoding' ||
              k === 'upgrade'
            ) {
              continue;
            }
            if (v !== undefined) outHeaders[k] = v;
          }
          resolve({
            status: res.statusCode || 502,
            headers: outHeaders,
            body: Buffer.concat(chunks),
          });
        });
      },
    );
    req.on('error', reject);
    if (reqBodyBuffer && reqBodyBuffer.length > 0) {
      req.write(reqBodyBuffer);
    }
    req.end();
  });
}

async function startH2WanProxy({
  apiUpstreamOrigin,
  webDistDir = null,
  listenHost = '127.0.0.1',
  listenPort = 0,
  downloadBytesPerSec = 150_000,
  rttMs = 60,
  burstBytes = 16_384,
  enableStaticBrotli = false,
} = {}) {
  if (!apiUpstreamOrigin) {
    throw new Error('startH2WanProxy requires apiUpstreamOrigin');
  }
  const resolvedWebRoot = webDistDir ? fs.realpathSync(webDistDir) : null;
  const oneWayDelayMs = Math.max(0, rttMs / 2);
  const shaper = createSharedEgressShaper({
    rateBytesPerSec: downloadBytesPerSec,
    burstBytes,
  });
  const tls = generateLoopbackTlsCredentials();
  const sessions = new Set();
  let epochMs = performance.now();
  const recordedRequests = [];

  const server = http2.createSecureServer(
    {
      key: tls.key,
      cert: tls.cert,
      allowHTTP1: true,
    },
    async (req, res) => {
      const startMs = performance.now() - epochMs;
      const method = req.method || 'GET';
      const rawUrl = req.url || '/';
      const parsedUrl = new URL(rawUrl, 'https://127.0.0.1');

      const bodyChunks = [];
      for await (const chunk of req) {
        bodyChunks.push(chunk);
      }
      const reqBody = Buffer.concat(bodyChunks);

      // Simulate client -> server uplink propagation delay (RTT / 2)
      if (oneWayDelayMs > 0) {
        await new Promise((resolve) => setTimeout(resolve, oneWayDelayMs));
      }
      if (res.destroyed || res.writableEnded) return;

      let upstreamResult;
      try {
        if (resolvedWebRoot && !isApiPath(parsedUrl.pathname)) {
          const staticRes = resolveStaticRepresentation(
            resolvedWebRoot,
            parsedUrl.pathname,
            req.headers,
            enableStaticBrotli,
          );
          if (staticRes.status === 404) {
            upstreamResult = {
              status: 404,
              headers: { 'content-type': 'text/plain; charset=utf-8' },
              body: Buffer.from('Not Found'),
            };
          } else if (
            (method === 'GET' || method === 'HEAD') &&
            staticRes.headers.etag &&
            req.headers['if-none-match'] === staticRes.headers.etag
          ) {
            upstreamResult = {
              status: 304,
              headers: {
                etag: staticRes.headers.etag,
                'cache-control': staticRes.headers['cache-control'],
              },
              body: Buffer.alloc(0),
            };
          } else {
            upstreamResult = staticRes;
          }
        } else {
          upstreamResult = await proxyHttpRequest(
            apiUpstreamOrigin,
            method,
            rawUrl,
            req.headers,
            reqBody,
          );
        }
      } catch (error) {
        upstreamResult = {
          status: 502,
          headers: { 'content-type': 'text/plain; charset=utf-8' },
          body: Buffer.from(`Bad Gateway: ${error.message}`),
        };
      }

      const responseReadyMs = performance.now() - epochMs;
      const payload = method === 'HEAD' || upstreamResult.status === 304
        ? Buffer.alloc(0)
        : upstreamResult.body;

      let firstByteSentMs = responseReadyMs;
      let endMs = responseReadyMs;

      await shaper.enqueue({
        body: payload,
        readyAt: performance.now() + oneWayDelayMs,
        isAborted: () => res.destroyed || res.writableEnded,
        onFirstByte: (now) => {
          firstByteSentMs = now - epochMs;
          if (!res.headersSent && !res.destroyed) {
            res.writeHead(upstreamResult.status, upstreamResult.headers);
          }
        },
        writeSlice: (slice) => {
          if (!res.destroyed && !res.writableEnded) {
            res.write(slice);
          }
        },
        finish: (now) => {
          endMs = now - epochMs;
          if (!res.destroyed && !res.writableEnded) {
            if (!res.headersSent) {
              res.writeHead(upstreamResult.status, upstreamResult.headers);
            }
            res.end();
          }
        },
      });

      recordedRequests.push({
        method,
        url: rawUrl,
        pathname: parsedUrl.pathname,
        status: upstreamResult.status,
        contentEncoding: String(upstreamResult.headers['content-encoding'] || 'identity'),
        wireBytes: payload.length,
        startMs: Number(startMs.toFixed(1)),
        responseReadyMs: Number(responseReadyMs.toFixed(1)),
        firstByteSentMs: Number(firstByteSentMs.toFixed(1)),
        endMs: Number(endMs.toFixed(1)),
        totalMs: Number((endMs - startMs).toFixed(1)),
        queueWaitMs: Number(Math.max(0, endMs - responseReadyMs - oneWayDelayMs).toFixed(1)),
      });
    },
  );

  server.on('session', (session) => {
    sessions.add(session);
    session.once('close', () => sessions.delete(session));
  });

  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(listenPort, listenHost, resolve);
  });

  const address = server.address();
  return {
    origin: `https://${listenHost}:${address.port}`,
    port: address.port,
    resetStats() {
      epochMs = performance.now();
      recordedRequests.length = 0;
      shaper.resetCounter();
    },
    getStats() {
      return {
        downloadBytesPerSec,
        rttMs,
        enableStaticBrotli,
        totalWireBytes: shaper.totalWireBytes,
        requests: [...recordedRequests],
      };
    },
    async close() {
      shaper.close();
      for (const session of [...sessions]) {
        session.destroy();
      }
      await new Promise((resolve) => server.close(resolve));
    },
  };
}

module.exports = {
  createSharedEgressShaper,
  startH2WanProxy,
};
