const net = require('node:net');

async function allocateDebugPort() {
  const server = net.createServer();
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  return port;
}

function summarizeNetwork(records) {
  const completed = records.filter((record) => record.end !== undefined);
  return {
    requestCount: records.length,
    completedCount: completed.length,
    encodedWireBytes: completed.reduce((sum, record) => sum + (record.encodedWireBytes ?? 0), 0),
    cachedCount: records.filter((record) => record.fromCache).length,
    workerRequests: records.filter((record) => /worker/u.test(record.targetType)).length,
    unfinished: records.filter((record) => record.end === undefined && !record.error)
      .map(({ url, type, targetType }) => ({ url, type, targetType })),
    failures: records.filter((record) => record.error || record.status >= 400)
      .map(({ url, status, error }) => ({ url, status, error }))
  };
}

// A separate native CDP socket auto-attaches to page AND Worker targets. Page
// Resource Timing alone misses Worker fetches. No request bodies, credentials,
// response bodies or sensitive headers are retained in the evidence.
async function captureBrowserNetwork(port) {
  const version = await fetch(`http://127.0.0.1:${port}/json/version`).then((r) => r.json());
  const socket = new WebSocket(version.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.addEventListener('open', resolve, { once: true }); socket.addEventListener('error', reject, { once: true }); });
  let serial = 0;
  let epoch = 0;
  const pending = new Map();
  const targets = new Map();
  const records = new Map();
  const instrumentationErrors = [];
  function send(method, params = {}, sessionId) {
    const id = ++serial;
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    });
  }
  socket.addEventListener('message', async ({ data }) => {
    const message = JSON.parse(String(data));
    if (message.id) {
      const flight = pending.get(message.id);
      pending.delete(message.id);
      if (message.error) flight?.reject(new Error(message.error.message));
      else flight?.resolve(message.result);
      return;
    }
    const { method, params: p, sessionId } = message;
    if (method === 'Target.attachedToTarget') {
      targets.set(p.sessionId, p.targetInfo);
      try {
        await send('Network.enable', {}, p.sessionId);
        await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true }, p.sessionId);
      } catch (error) {
        if (!/closed|detached|not found/iu.test(error.message)) instrumentationErrors.push(error.message);
      } finally {
        await send('Runtime.runIfWaitingForDebugger', {}, p.sessionId).catch(() => {});
      }
      return;
    }
    if (!method?.startsWith('Network.')) return;
    const key = `${sessionId}:${p.requestId}`;
    if (method === 'Network.requestWillBeSent') {
      const previous = records.get(key);
      if (previous && p.redirectResponse) {
        previous.status = p.redirectResponse.status;
        previous.end = p.timestamp;
        previous.encodedWireBytes = p.redirectResponse.encodedDataLength;
        records.set(`${key}:redirect:${previous.start}`, previous);
      }
      const url = new URL(p.request.url);
      // Keep path-level evidence; authentication query parameters are not needed.
      url.search = '';
      records.set(key, {
        epoch, url: url.href, method: p.request.method, type: p.type,
        targetType: targets.get(sessionId)?.type ?? 'unknown',
        targetId: targets.get(sessionId)?.targetId,
        start: p.timestamp, wallTime: p.wallTime,
        initiator: { type: p.initiator?.type, url: p.initiator?.url ?? p.initiator?.stack?.callFrames?.[0]?.url }
      });
    }
    const record = records.get(key);
    if (!record) return;
    if (method === 'Network.requestServedFromCache') record.fromCache = true;
    if (method === 'Network.responseReceived') {
      const r = p.response;
      record.status = r.status;
      record.protocol = r.protocol;
      record.fromCache ||= Boolean(r.fromDiskCache || r.fromPrefetchCache || r.fromServiceWorker);
      record.timing = r.timing;
      record.headers = Object.fromEntries(Object.entries(r.headers).filter(([name]) => /^(content-encoding|content-length|cache-control|etag|last-modified)$/iu.test(name)));
      record.response = p.timestamp;
    }
    if (method === 'Network.loadingFinished') {
      record.end = p.timestamp;
      record.encodedWireBytes = p.encodedDataLength;
    }
    if (method === 'Network.loadingFailed') {
      record.error = p.errorText;
      record.end = p.timestamp;
    }
  });
  await send('Target.setAutoAttach', { autoAttach: true, waitForDebuggerOnStart: true, flatten: true });
  return {
    startMeasurement() { epoch += 1; return epoch; },
    snapshot() {
      const requests = [...records.values()].filter((record) => record.epoch === epoch);
      return { browser: version.Browser, targets: [...targets.values()].map(({ type, targetId }) => ({ type, targetId })), requests, summary: summarizeNetwork(requests), instrumentationErrors: [...instrumentationErrors] };
    },
    async dispose() {
      await send('Target.setAutoAttach', { autoAttach: false, waitForDebuggerOnStart: false, flatten: true }).catch(() => {});
      socket.close();
      for (const flight of pending.values()) flight.reject(new Error('Network capture closed'));
      pending.clear();
    }
  };
}
module.exports = { allocateDebugPort, captureBrowserNetwork, summarizeNetwork };
