#!/usr/bin/env node
'use strict';

const { spawn } = require('node:child_process');
const fs = require('node:fs');
const http = require('node:http');
const net = require('node:net');
const path = require('node:path');
const { createRequire } = require('node:module');

const {
  loadRootCredentials,
  openTemporaryConsoleSession,
  openTemporaryOwnerSession,
  rebaseStorageStateCookies,
} = require('../auth.js');
const { buildServiceEnv, getServiceDefinitions } = require('../../dev-up/core.js');
const { startH2WanProxy } = require('./h2-wan-proxy.js');
const { rewriteDatabaseUrlForProxy, startPgDelayProxy } = require('./pg-delay-proxy.js');

function parseArgs(argv) {
  const opts = {
    label: 'run',
    apiOrigin: null,
    apiBin: null,
    databaseUrl: null,
    webDist: null,
    useApiStaticWebRoot: false,
    blocks: 20,
    downBps: 150_000,
    rttMs: 60,
    pgRttMs: 3,
    staticBrotli: false,
    timeoutMs: 90_000,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const next = () => {
      if (i + 1 >= argv.length) throw new Error(`Missing value for ${arg}`);
      i += 1;
      return argv[i];
    };
    if (arg === '--label') opts.label = next();
    else if (arg === '--api-origin') opts.apiOrigin = next();
    else if (arg === '--api-bin') opts.apiBin = next();
    else if (arg === '--database-url') opts.databaseUrl = next();
    else if (arg === '--web-dist') opts.webDist = next();
    else if (arg === '--use-api-static-web-root') opts.useApiStaticWebRoot = true;
    else if (arg === '--blocks') opts.blocks = Number(next());
    else if (arg === '--down-bps') opts.downBps = Number(next());
    else if (arg === '--rtt-ms') opts.rttMs = Number(next());
    else if (arg === '--pg-rtt-ms') opts.pgRttMs = Number(next());
    else if (arg === '--static-br') opts.staticBrotli = true;
    else if (arg === '--timeout-ms') opts.timeoutMs = Number(next());
    else if (arg === '--help') {
      console.log(
        'Usage: node scripts/node/page-debug/wan-benchmark/run-frontstage-wan-benchmark.js ' +
          '--label <name> [--api-bin <path> | --api-origin <url>] [--web-dist <dir>] ' +
          '[--blocks 20] [--down-bps 150000] [--rtt-ms 60] [--pg-rtt-ms 3] [--static-br] [--use-api-static-web-root]',
      );
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  if (!/^[a-zA-Z0-9_-]+$/.test(opts.label)) {
    throw new Error(`Invalid --label: ${opts.label}`);
  }
  return opts;
}

async function allocateFreeTcpPort(host = '127.0.0.1') {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.once('error', reject);
    srv.listen(0, host, () => {
      const { port } = srv.address();
      srv.close(() => resolve(port));
    });
  });
}

async function waitForHttpHealth(origin, deadlineMs = 45_000) {
  const deadline = Date.now() + deadlineMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`${origin}/health`, {
        signal: AbortSignal.timeout(2_500),
      });
      if (res.ok) return;
    } catch {
      // retry
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(`Backend at ${origin} did not become healthy within ${deadlineMs}ms`);
}

function buildBlockSourceCode(index) {
  const tag = String(index).padStart(2, '0');
  // Generate realistic ~8.5 KB TSX component source per block (20 blocks ≈ 170 KB raw TSX)
  const metricRows = Array.from({ length: 48 }, (_, row) => {
    return `  { key: 'm_${tag}_${row}', label: '指标项 ${tag}-${row}', baseline: ${(index + 1) * 100 + row * 7}, delta: '+${(row % 9) + 1}.${index % 9}%', note: '实时采样窗口聚合序列 ${tag}-${row} 用于验证公网带宽下数据库动态 TSX 传输与视口分级编译吞吐表现' },`;
  }).join('\n');

  return `import React, { useMemo } from 'react';
import { Card, Statistic, Tag, Typography } from 'antd';

const METRIC_SERIES_${tag} = [
${metricRows}
];

export default function WanBenchmarkBlock${tag}() {
  const summary = useMemo(() => {
    let total = 0;
    for (const item of METRIC_SERIES_${tag}) {
      total += item.baseline;
    }
    return { total, count: METRIC_SERIES_${tag}.length, head: METRIC_SERIES_${tag}[0] };
  }, []);

  return (
    <Card size="small" title={\`监控区块 #${tag}\`} extra={<Tag color="processing">LIVE-${tag}</Tag>}>
      <div data-wan-bench-block="WAN-BLOCK-${tag}">
        <Statistic title={summary.head.label} value={summary.total} suffix={\`(\${summary.count})\`} />
        <Typography.Text type="secondary">{summary.head.delta}</Typography.Text>
      </div>
    </Card>
  );
}
`;
}

async function apiJson(apiOrigin, session, method, urlPath, body) {
  const headers = {
    cookie: session.cookie,
    'x-csrf-token': session.csrfToken,
  };
  if (body !== undefined) {
    headers['content-type'] = 'application/json';
  }
  const response = await fetch(`${apiOrigin}${urlPath}`, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`${method} ${urlPath} failed (${response.status}): ${text.slice(0, 400)}`);
  }
  return text ? JSON.parse(text) : null;
}

async function seedBenchmarkFrontstagePage(apiOrigin, session, blockCount) {
  const catalogRes = await apiJson(
    apiOrigin,
    session,
    'GET',
    '/api/console/frontend-blocks?registration_target=block',
  );
  const catalogEntries = Array.isArray(catalogRes?.data) ? catalogRes.data : [];
  const nativeEntry = catalogEntries.find(
    (entry) =>
      !entry.disable_reason &&
      entry.runtime_kind === 'trusted_native' &&
      Boolean(entry.code_template_version),
  );
  if (!nativeEntry) {
    throw new Error('No active trusted_native JS block catalog entry found in workspace');
  }

  const suffix = `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
  const slug = `wan-bench-${suffix}`;
  const created = await apiJson(apiOrigin, session, 'POST', '/api/console/frontstage/pages', {
    title: `WAN Benchmark ${suffix}`,
    placement: 'topbar',
    slug,
  });
  const pageId = created.data.page.id;
  const tabId = created.data.default_tab.id;

  // Layout: 2 columns x 12 grid units wide, each row 8 grid units tall (~320px/row).
  // Slots 0..3 (rows 0..1) are inside initial 900px viewport; slots 4..5 near-viewport; slots 6..19 offscreen (>400px).
  for (let i = 0; i < blockCount; i += 1) {
    const col = (i % 2) * 12;
    const row = Math.floor(i / 2) * 8;
    const layoutBox = { x: col, y: row, w: 12, h: 8 };
    await apiJson(
      apiOrigin,
      session,
      'POST',
      `/api/console/frontstage/pages/${pageId}/blocks`,
      {
        tab_id: tabId,
        title: `WAN Block ${String(i).padStart(2, '0')}`,
        presentation: 'page',
        source_code: buildBlockSourceCode(i),
        runtime_descriptor: {
          renderer_version: 'v1',
          catalog: {
            providerCode: nativeEntry.provider_code,
            installationId: nativeEntry.installation_id,
          },
          contribution: {
            pluginId: nativeEntry.plugin_id,
            pluginVersion: nativeEntry.plugin_version,
            code: nativeEntry.contribution_code,
          },
          props: {},
          ports: { inputs: [], outputs: [] },
          'x-presentation': { heightMode: 'auto', height: null },
          'x-layout': {
            order: i,
            region: 'main',
            gridColumns: 24,
            verticalGridVersion: 1,
            ...layoutBox,
            lg: layoutBox,
            md: layoutBox,
          },
          runtime: {
            kind: nativeEntry.runtime,
            entry: nativeEntry.entry,
            hint: nativeEntry.runtime,
            code_template_version: nativeEntry.code_template_version,
            code_template_language: nativeEntry.code_template_language || 'tsx',
          },
        },
      },
    );
  }

  return {
    slug,
    pageId,
    tabId,
    async cleanup() {
      await apiJson(apiOrigin, session, 'DELETE', `/api/console/frontstage/pages/${pageId}`);
    },
  };
}

async function measurePass({
  page,
  cdp,
  targetUrl,
  isReload,
  h2Proxy,
  pgProxy,
  timeoutMs,
}) {
  const cdpRecords = [];
  const inFlight = new Map();
  let passEpochMs = Date.now();

  const onReq = (e) => {
    if (!/^https?:/.test(e.request.url)) return;
    const rec = {
      requestId: e.requestId,
      url: e.request.url,
      pathname: new URL(e.request.url).pathname,
      method: e.request.method,
      type: e.type,
      startMs: Date.now() - passEpochMs,
      status: null,
      fromDiskCache: false,
      fromMemoryCache: false,
      encodedBytes: 0,
      endMs: null,
      durationMs: null,
    };
    cdpRecords.push(rec);
    inFlight.set(e.requestId, rec);
  };
  const onRes = (e) => {
    const rec = inFlight.get(e.requestId);
    if (!rec) return;
    rec.status = e.response.status;
    rec.fromDiskCache = Boolean(e.response.fromDiskCache);
    rec.fromMemoryCache = Boolean(e.response.fromPrefetchCache);
    rec.contentEncoding = e.response.headers?.['content-encoding'] || 'identity';
  };
  const onDone = (e) => {
    const rec = inFlight.get(e.requestId);
    if (!rec) return;
    rec.endMs = Date.now() - passEpochMs;
    rec.durationMs = rec.endMs - rec.startMs;
    rec.encodedBytes = e.encodedDataLength || 0;
    inFlight.delete(e.requestId);
  };

  cdp.on('Network.requestWillBeSent', onReq);
  cdp.on('Network.responseReceived', onRes);
  cdp.on('Network.loadingFinished', onDone);
  cdp.on('Network.loadingFailed', onDone);

  try {
    h2Proxy.resetStats();
    pgProxy?.resetStats();
    passEpochMs = Date.now();

    const navStart = performance.now();
    if (isReload) {
      await page.reload({ waitUntil: 'domcontentloaded', timeout: timeoutMs });
    } else {
      await page.goto(targetUrl, { waitUntil: 'domcontentloaded', timeout: timeoutMs });
    }

    // Wait for visible top-row compiled custom TSX blocks (WAN-BLOCK-00 & WAN-BLOCK-01) to mount on screen
    await page.locator('[data-wan-bench-block="WAN-BLOCK-00"]').waitFor({
      state: 'visible',
      timeout: timeoutMs,
    });
    await page.locator('[data-wan-bench-block="WAN-BLOCK-01"]').waitFor({
      state: 'visible',
      timeout: timeoutMs,
    });
    const visibleBlocksReadyMs = Number((performance.now() - navStart).toFixed(1));

    // Allow trailing non-SSE startup requests to settle (bounded wait)
    const settleDeadline = Date.now() + 8_000;
    while (Date.now() < settleDeadline) {
      const pendingNonStream = [...inFlight.values()].filter(
        (r) => !r.pathname.includes('/notifications/events') && !r.pathname.includes('/websocket'),
      );
      if (pendingNonStream.length === 0) break;
      await new Promise((r) => setTimeout(r, 120));
    }
    const networkSettleMs = Number((performance.now() - navStart).toFixed(1));

    const mountedBlockMarkers = await page
      .locator('[data-wan-bench-block]')
      .evaluateAll((els) => els.map((el) => el.getAttribute('data-wan-bench-block')));

    const proxyStats = h2Proxy.getStats();
    const pgStats = pgProxy ? pgProxy.getStats() : null;

    const networkTransferred = cdpRecords.filter(
      (r) => !r.fromDiskCache && r.durationMs !== null,
    );
    const smallRequests = proxyStats.requests.filter(
      (r) => r.wireBytes > 0 && r.wireBytes <= 2048,
    );
    const workerRequests = cdpRecords.filter((r) =>
      r.pathname.includes('native-react-compiler.worker'),
    );
    const workerNetworkRequests = workerRequests.filter((r) => !r.fromDiskCache);
    const enUsRequests = cdpRecords.filter(
      (r) => /(?:\b|\/|-)en_US[-.]|i18n-shell-en-/u.test(r.pathname) && !r.fromDiskCache,
    );
    const bootstrapCdp = cdpRecords.find((r) =>
      r.pathname.endsWith('/api/console/frontstage/bootstrap'),
    );
    const bootstrapServer = proxyStats.requests.find((r) =>
      r.pathname.endsWith('/api/console/frontstage/bootstrap'),
    );

    return {
      visibleBlocksReadyMs,
      networkSettleMs,
      mountedBlockCountAtReady: mountedBlockMarkers.length,
      mountedBlockMarkers,
      totalClientRequests: cdpRecords.length,
      networkTransferredRequests: networkTransferred.length,
      diskCachedRequests: cdpRecords.filter((r) => r.fromDiskCache).length,
      totalServerWireBytes: proxyStats.totalWireBytes,
      maxServerRequestTotalMs: proxyStats.requests.reduce(
        (max, r) => Math.max(max, r.totalMs),
        0,
      ),
      smallRequestMaxTotalMs: smallRequests.reduce(
        (max, r) => Math.max(max, r.totalMs),
        0,
      ),
      smallRequestSamples: smallRequests.slice(0, 10),
      bootstrap: bootstrapCdp
        ? {
            clientStartMs: bootstrapCdp.startMs,
            clientDurationMs: bootstrapCdp.durationMs,
            status: bootstrapCdp.status,
            contentEncoding: bootstrapServer?.contentEncoding ?? bootstrapCdp.contentEncoding,
            wireBytes: bootstrapServer?.wireBytes ?? bootstrapCdp.encodedBytes,
            serverResponseReadyMs: bootstrapServer
              ? Number((bootstrapServer.responseReadyMs - bootstrapServer.startMs).toFixed(1))
              : null,
          }
        : null,
      workerTotalRequests: workerRequests.length,
      workerNetworkRequests: workerNetworkRequests.length,
      workerNetworkBytes: workerNetworkRequests.reduce((s, r) => s + r.encodedBytes, 0),
      enUsNetworkRequests: enUsRequests.length,
      enUsNetworkBytes: enUsRequests.reduce((s, r) => s + r.encodedBytes, 0),
      postgres: pgStats,
      topSlowestServerRequests: [...proxyStats.requests]
        .sort((a, b) => b.totalMs - a.totalMs)
        .slice(0, 15),
    };
  } finally {
    cdp.off('Network.requestWillBeSent', onReq);
    cdp.off('Network.responseReceived', onRes);
    cdp.off('Network.loadingFinished', onDone);
    cdp.off('Network.loadingFailed', onDone);
  }
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const repoRoot = path.resolve(__dirname, '../../../..');
  const outDir = path.join(repoRoot, 'tmp', 'test-governance', 'wan-benchmark', opts.label);
  fs.mkdirSync(outDir, { recursive: true });

  const webDist = opts.webDist
    ? path.resolve(opts.webDist)
    : path.join(repoRoot, 'web', 'app', 'dist');
  if (!fs.existsSync(path.join(webDist, 'index.html'))) {
    throw new Error(`Missing built web/app/dist at ${webDist}`);
  }

  const playwright = createRequire(path.join(repoRoot, 'web', 'package.json'))('playwright');
  const chromiumExecutable =
    process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH ||
    (['/usr/bin/google-chrome', '/usr/bin/chromium', '/usr/bin/chromium-browser'].find((p) =>
      fs.existsSync(p),
    ) ??
      undefined);

  let pgProxy = null;
  let apiChild = null;
  let apiOrigin = opts.apiOrigin;
  let ownerSession = null;
  let consoleSession = null;
  let seededPage = null;
  let h2Proxy = null;
  let browser = null;

  try {
    if (opts.apiBin) {
      const apiService = getServiceDefinitions(repoRoot)['api-server'];
      const baseServiceEnv = buildServiceEnv(apiService, process.env);
      const rawDbUrl =
        opts.databaseUrl || baseServiceEnv.API_DATABASE_URL || baseServiceEnv.DATABASE_URL;
      if (!rawDbUrl) {
        throw new Error('Missing API_DATABASE_URL / DATABASE_URL for --api-bin');
      }
      const parsedDb = new URL(rawDbUrl);
      pgProxy = await startPgDelayProxy({
        upstreamHost: parsedDb.hostname || '127.0.0.1',
        upstreamPort: Number(parsedDb.port || '5432'),
        rttMs: opts.pgRttMs,
      });
      const { proxiedDatabaseUrl } = rewriteDatabaseUrlForProxy(rawDbUrl, pgProxy.port);
      const apiPort = await allocateFreeTcpPort('127.0.0.1');
      apiOrigin = `http://127.0.0.1:${apiPort}`;

      const childEnv = {
        ...baseServiceEnv,
        API_DATABASE_URL: proxiedDatabaseUrl,
        DATABASE_URL: proxiedDatabaseUrl,
        API_SERVER_ADDR: `127.0.0.1:${apiPort}`,
        ...(opts.useApiStaticWebRoot ? { API_STATIC_WEB_ROOT: webDist } : {}),
      };
      const apiLogStream = fs.createWriteStream(path.join(outDir, 'api-server.log'));
      apiChild = spawn(path.resolve(opts.apiBin), [], {
        cwd: path.join(repoRoot, 'api'),
        env: childEnv,
        stdio: ['ignore', 'pipe', 'pipe'],
      });
      apiChild.stdout.pipe(apiLogStream);
      apiChild.stderr.pipe(apiLogStream);
      await waitForHttpHealth(apiOrigin, 60_000);
    } else if (!apiOrigin) {
      throw new Error('Provide either --api-bin <path> or --api-origin <url>');
    } else {
      await waitForHttpHealth(apiOrigin, 30_000);
    }

    const creds = loadRootCredentials({ repoRoot });
    ownerSession = await openTemporaryOwnerSession({
      apiBaseUrl: apiOrigin,
      account: creds.account,
      password: creds.password,
    });

    seededPage = await seedBenchmarkFrontstagePage(apiOrigin, ownerSession, opts.blocks);

    // Explicitly measure isolated single `GET /api/console/frontstage/bootstrap` wire + SQL cost
    const rawHttpGet = (fullUrl, reqHeaders) =>
      new Promise((resolve, reject) => {
        const req = http.request(fullUrl, { method: 'GET', headers: reqHeaders }, (res) => {
          const chunks = [];
          res.on('data', (c) => chunks.push(c));
          res.on('end', () =>
            resolve({
              status: res.statusCode,
              headers: res.headers,
              wireBuffer: Buffer.concat(chunks),
            }),
          );
        });
        req.on('error', reject);
        req.end();
      });

    let directBootstrapProbe = null;
    if (pgProxy) pgProxy.resetStats();
    const bootRes = await rawHttpGet(
      `${apiOrigin}/api/console/frontstage/bootstrap?slug=${encodeURIComponent(seededPage.slug)}&code_limit=12`,
      {
        cookie: ownerSession.cookie,
        'accept-encoding': 'gzip',
        'accept-language': 'zh-CN,zh;q=0.9',
      },
    );
    const bootPg = pgProxy ? pgProxy.getStats() : null;
    const bootEtag = bootRes.headers['etag'] || null;
    let boot304Status = null;
    let boot304WireBytes = null;
    let boot304Pg = null;
    if (bootEtag) {
      if (pgProxy) pgProxy.resetStats();
      const res304 = await rawHttpGet(
        `${apiOrigin}/api/console/frontstage/bootstrap?slug=${encodeURIComponent(seededPage.slug)}&code_limit=12`,
        {
          cookie: ownerSession.cookie,
          'if-none-match': bootEtag,
          'accept-encoding': 'gzip',
          'accept-language': 'zh-CN,zh;q=0.9',
        },
      );
      boot304Status = res304.status;
      boot304WireBytes = res304.wireBuffer.length;
      boot304Pg = pgProxy ? pgProxy.getStats() : null;
    }
    directBootstrapProbe = {
      status: bootRes.status,
      contentEncoding: bootRes.headers['content-encoding'] || 'identity',
      etag: bootEtag,
      wireBytes: bootRes.wireBuffer.length,
      postgres: bootPg,
      revalidation304: bootEtag
        ? { status: boot304Status, wireBytes: boot304WireBytes, postgres: boot304Pg }
        : null,
    };

    h2Proxy = await startH2WanProxy({
      apiUpstreamOrigin: apiOrigin,
      webDistDir: opts.useApiStaticWebRoot ? null : webDist,
      downloadBytesPerSec: opts.downBps,
      rttMs: opts.rttMs,
      enableStaticBrotli: opts.staticBrotli,
    });

    const storageStatePath = path.join(outDir, 'storage-state.json');
    consoleSession = await openTemporaryConsoleSession({
      playwright,
      apiBaseUrl: apiOrigin,
      account: creds.account,
      password: creds.password,
      storageStatePath,
    });
    rebaseStorageStateCookies(storageStatePath, h2Proxy.origin);

    browser = await playwright.chromium.launch({
      headless: true,
      ...(chromiumExecutable ? { executablePath: chromiumExecutable } : {}),
    });

    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      locale: 'zh-CN',
      ignoreHTTPSErrors: true,
      serviceWorkers: 'block',
      storageState: storageStatePath,
    });
    const page = await context.newPage();
    const cdp = await context.newCDPSession(page);
    await cdp.send('Network.enable');

    const targetUrl = `${h2Proxy.origin}/${seededPage.slug}`;
    const cold = await measurePass({
      page,
      cdp,
      targetUrl,
      isReload: false,
      h2Proxy,
      pgProxy,
      timeoutMs: opts.timeoutMs,
    });

    const warm = await measurePass({
      page,
      cdp,
      targetUrl,
      isReload: true,
      h2Proxy,
      pgProxy,
      timeoutMs: opts.timeoutMs,
    });

    const report = {
      label: opts.label,
      generatedAt: new Date().toISOString(),
      environment: {
        downBytesPerSec: opts.downBps,
        downMbps: Number(((opts.downBps * 8) / 1_000_000).toFixed(2)),
        wanRttMs: opts.rttMs,
        pgRttMs: pgProxy ? opts.pgRttMs : null,
        blockCount: opts.blocks,
        staticBrotli: opts.staticBrotli,
        useApiStaticWebRoot: opts.useApiStaticWebRoot,
      },
      directBootstrapProbe,
      cold,
      warm,
    };

    const reportPath = path.join(outDir, 'report.json');
    fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
    if (fs.existsSync(storageStatePath)) {
      fs.unlinkSync(storageStatePath);
    }

    console.log(
      JSON.stringify(
        {
          reportPath,
          environment: report.environment,
          directBootstrapProbe,
          coldSummary: {
            visibleBlocksReadyMs: cold.visibleBlocksReadyMs,
            networkSettleMs: cold.networkSettleMs,
            mountedBlockCountAtReady: cold.mountedBlockCountAtReady,
            totalServerWireBytes: cold.totalServerWireBytes,
            maxServerRequestTotalMs: cold.maxServerRequestTotalMs,
            smallRequestMaxTotalMs: cold.smallRequestMaxTotalMs,
            bootstrap: cold.bootstrap,
            workerNetworkRequests: cold.workerNetworkRequests,
            workerNetworkBytes: cold.workerNetworkBytes,
            enUsNetworkRequests: cold.enUsNetworkRequests,
            enUsNetworkBytes: cold.enUsNetworkBytes,
            postgres: cold.postgres,
          },
          warmSummary: {
            visibleBlocksReadyMs: warm.visibleBlocksReadyMs,
            networkSettleMs: warm.networkSettleMs,
            mountedBlockCountAtReady: warm.mountedBlockCountAtReady,
            networkTransferredRequests: warm.networkTransferredRequests,
            diskCachedRequests: warm.diskCachedRequests,
            totalServerWireBytes: warm.totalServerWireBytes,
            bootstrap: warm.bootstrap,
            workerTotalRequests: warm.workerTotalRequests,
            workerNetworkRequests: warm.workerNetworkRequests,
            enUsNetworkRequests: warm.enUsNetworkRequests,
            postgres: warm.postgres,
          },
        },
        null,
        2,
      ),
    );
  } finally {
    if (browser) {
      await browser.close().catch(() => undefined);
    }
    if (h2Proxy) {
      await h2Proxy.close().catch(() => undefined);
    }
    if (seededPage) {
      await seededPage.cleanup().catch(() => undefined);
    }
    if (consoleSession) {
      await consoleSession.dispose().catch(() => undefined);
    }
    if (ownerSession) {
      await ownerSession.dispose().catch(() => undefined);
    }
    if (apiChild) {
      apiChild.kill('SIGTERM');
      await new Promise((resolve) => {
        const timer = setTimeout(() => {
          apiChild.kill('SIGKILL');
          resolve();
        }, 5_000);
        apiChild.once('exit', () => {
          clearTimeout(timer);
          resolve();
        });
      });
    }
    if (pgProxy) {
      await pgProxy.close().catch(() => undefined);
    }
  }
}

if (require.main === module) {
  main().catch((error) => {
    console.error('[wan-benchmark] fatal:', error);
    process.exit(1);
  });
}

module.exports = {
  buildBlockSourceCode,
  parseArgs,
};
