'use strict';

const net = require('node:net');

const PG_SSL_REQUEST_CODE = 80877103;
const PG_CANCEL_REQUEST_CODE = 80877102;
const PG_PROTOCOL_V3 = 196608;

/**
 * Passive, non-mutating PostgreSQL v3 frontend wire-frame counter.
 * Never alters forwarded TCP bytes; inspects a copy of client->server chunks.
 */
function createPgFrontendFrameTap(onCounters) {
  let buffer = Buffer.alloc(0);
  let phase = 'startup'; // 'startup' | 'messages' | 'passthrough'

  return function pushChunk(chunk) {
    if (phase === 'passthrough' || !chunk || chunk.length === 0) {
      return;
    }
    buffer = buffer.length === 0 ? chunk : Buffer.concat([buffer, chunk]);

    while (buffer.length > 0) {
      if (phase === 'startup') {
        if (buffer.length < 8) return;
        const length = buffer.readInt32BE(0);
        const code = buffer.readInt32BE(4);
        if (length < 8 || length > 1024 * 1024) {
          phase = 'passthrough';
          buffer = Buffer.alloc(0);
          return;
        }
        if (buffer.length < length) return;
        buffer = buffer.subarray(length);
        if (code === PG_SSL_REQUEST_CODE) {
          // If client stays plaintext ('N'), another StartupMessage follows.
          continue;
        }
        if (code === PG_CANCEL_REQUEST_CODE) {
          phase = 'passthrough';
          buffer = Buffer.alloc(0);
          return;
        }
        if (code === PG_PROTOCOL_V3) {
          phase = 'messages';
          continue;
        }
        phase = 'passthrough';
        buffer = Buffer.alloc(0);
        return;
      }

      // Standard v3 frontend frame: [type: 1B][len: 4B BE, includes self][payload: len - 4]
      if (buffer.length < 5) return;
      const msgType = buffer[0];
      const msgLen = buffer.readInt32BE(1);
      if (msgLen < 4 || msgLen > 64 * 1024 * 1024) {
        phase = 'passthrough';
        buffer = Buffer.alloc(0);
        return;
      }
      if (buffer.length < 1 + msgLen) return;

      // 'Q' (0x51): Simple Query (1 execution + 1 round-trip)
      // 'E' (0x45): Extended Query Execute (1 execution)
      // 'S' (0x53): Extended Query Sync (1 network flush / round-trip)
      if (msgType === 0x51) {
        onCounters({ executions: 1, roundTrips: 1 });
      } else if (msgType === 0x45) {
        onCounters({ executions: 1, roundTrips: 0 });
      } else if (msgType === 0x53) {
        onCounters({ executions: 0, roundTrips: 1 });
      }

      buffer = buffer.subarray(1 + msgLen);
    }
  };
}

/**
 * FIFO order-preserving one-way latency pipe for a single TCP half-duplex direction.
 */
function forwardWithOneWayDelay(source, dest, oneWayDelayMs, onChunk) {
  if (oneWayDelayMs <= 0) {
    source.on('data', (chunk) => {
      if (onChunk) onChunk(chunk);
      if (!dest.destroyed) dest.write(chunk);
    });
    source.on('end', () => {
      if (!dest.destroyed) dest.end();
    });
    return () => {};
  }

  const queue = [];
  let timer = null;
  let ended = false;

  const pump = () => {
    timer = null;
    const now = performance.now();
    while (queue.length > 0 && queue[0].readyAt <= now) {
      const item = queue.shift();
      if (!dest.destroyed) {
        dest.write(item.chunk);
      }
    }
    if (queue.length > 0) {
      const waitMs = Math.max(1, Math.ceil(queue[0].readyAt - performance.now()));
      timer = setTimeout(pump, waitMs);
    } else if (ended && !dest.destroyed) {
      dest.end();
    }
  };

  source.on('data', (chunk) => {
    if (onChunk) onChunk(chunk);
    const copy = Buffer.from(chunk);
    queue.push({ chunk: copy, readyAt: performance.now() + oneWayDelayMs });
    if (!timer) {
      timer = setTimeout(pump, Math.max(1, Math.ceil(oneWayDelayMs)));
    }
  });

  source.on('end', () => {
    ended = true;
    if (queue.length === 0 && !dest.destroyed) {
      dest.end();
    }
  });

  return () => {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
    queue.length = 0;
  };
}

function rewriteDatabaseUrlForProxy(databaseUrl, proxyPort, proxyHost = '127.0.0.1') {
  const parsed = new URL(databaseUrl);
  const upstreamHost = parsed.hostname || '127.0.0.1';
  const upstreamPort = Number(parsed.port || '5432');
  parsed.hostname = proxyHost;
  parsed.port = String(proxyPort);
  parsed.searchParams.set('sslmode', 'disable');
  return {
    upstreamHost,
    upstreamPort,
    proxiedDatabaseUrl: parsed.toString(),
  };
}

async function startPgDelayProxy({
  upstreamHost = '127.0.0.1',
  upstreamPort = 5432,
  listenHost = '127.0.0.1',
  listenPort = 0,
  rttMs = 3,
} = {}) {
  const oneWayDelayMs = Math.max(0, rttMs / 2);
  const sockets = new Set();
  const cleanups = new Set();
  let sqlExecutions = 0;
  let sqlRoundTrips = 0;
  let clientBytes = 0;
  let serverBytes = 0;

  const server = net.createServer((clientSocket) => {
    clientSocket.setNoDelay(true);
    const upstreamSocket = net.connect(upstreamPort, upstreamHost);
    upstreamSocket.setNoDelay(true);

    sockets.add(clientSocket);
    sockets.add(upstreamSocket);

    const tap = createPgFrontendFrameTap(({ executions, roundTrips }) => {
      sqlExecutions += executions;
      sqlRoundTrips += roundTrips;
    });

    const cancelClientToServer = forwardWithOneWayDelay(
      clientSocket,
      upstreamSocket,
      oneWayDelayMs,
      (chunk) => {
        clientBytes += chunk.length;
        tap(chunk);
      },
    );
    const cancelServerToClient = forwardWithOneWayDelay(
      upstreamSocket,
      clientSocket,
      oneWayDelayMs,
      (chunk) => {
        serverBytes += chunk.length;
      },
    );

    const teardown = () => {
      cancelClientToServer();
      cancelServerToClient();
      cleanups.delete(teardown);
      sockets.delete(clientSocket);
      sockets.delete(upstreamSocket);
      if (!clientSocket.destroyed) clientSocket.destroy();
      if (!upstreamSocket.destroyed) upstreamSocket.destroy();
    };
    cleanups.add(teardown);

    clientSocket.on('error', teardown);
    upstreamSocket.on('error', teardown);
    clientSocket.once('close', teardown);
    upstreamSocket.once('close', teardown);
  });

  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(listenPort, listenHost, resolve);
  });

  const address = server.address();
  return {
    host: listenHost,
    port: address.port,
    rttMs,
    resetStats() {
      sqlExecutions = 0;
      sqlRoundTrips = 0;
      clientBytes = 0;
      serverBytes = 0;
    },
    getStats() {
      return {
        rttMs,
        sqlExecutions,
        sqlRoundTrips,
        clientBytes,
        serverBytes,
      };
    },
    async close() {
      for (const cleanup of [...cleanups]) cleanup();
      for (const socket of [...sockets]) socket.destroy();
      await new Promise((resolve) => server.close(resolve));
    },
  };
}

module.exports = {
  createPgFrontendFrameTap,
  rewriteDatabaseUrlForProxy,
  startPgDelayProxy,
};
