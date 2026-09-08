const http = require("node:http");
const net = require("node:net");
const { Transform } = require("node:stream");

function budget(rate, latency) {
  let tokens = 16384,
    last = performance.now(),
    bytes = 0;
  const pending = [];
  const timer = setInterval(() => {
    const now = performance.now();
    tokens = Math.min(16384, tokens + ((now - last) * rate) / 1000);
    last = now;
    let visits = pending.length;
    while (pending.length && visits-- > 0 && tokens >= 1) {
      const job = pending.shift();
      if (job.stream.destroyed) {
        job.done();
        continue;
      }
      if (job.ready > now) {
        pending.push(job);
        continue;
      }
      const size = Math.min(
        Math.floor(tokens),
        16384,
        job.chunk.length - job.offset,
      );
      job.stream.push(job.chunk.subarray(job.offset, job.offset + size));
      job.offset += size;
      tokens -= size;
      bytes += size;
      if (job.offset === job.chunk.length) job.done();
      else pending.push(job);
    }
  }, 10);
  return {
    get bytes() {
      return bytes;
    },
    stream: () =>
      new Transform({
        transform(chunk, _encoding, done) {
          pending.push({
            chunk,
            offset: 0,
            ready: performance.now() + latency,
            stream: this,
            done,
          });
        },
      }),
    close: () => {
      clearInterval(timer);
      for (const job of pending) job.done();
      pending.length = 0;
    },
  };
}
async function startNetworkProxy() {
  const down = budget(200000, 75),
    up = budget(93750, 75);
  const sockets = new Set();
  const track = (socket) => {
    sockets.add(socket);
    socket.once("close", () => sockets.delete(socket));
    return socket;
  };
  const server = http.createServer((request, response) => {
    let url;
    try {
      url = new URL(request.url);
    } catch {
      response.writeHead(400).end();
      return;
    }
    const headers = { ...request.headers };
    delete headers["proxy-connection"];
    const outbound = http.request(
      url,
      { method: request.method, headers },
      (incoming) => {
        const delay = setTimeout(() => {
          if (response.destroyed) return;
          response.writeHead(incoming.statusCode, incoming.headers);
          incoming.pipe(down.stream()).pipe(response);
        }, 75);
        response.once("close", () => {
          clearTimeout(delay);
          incoming.destroy();
        });
      },
    );
    outbound.on("socket", track);
    outbound.on("error", () => {
      if (!response.headersSent) response.writeHead(502);
      response.end();
    });
    response.once("close", () => outbound.destroy());
    request.pipe(up.stream()).pipe(outbound);
  });
  server.on("connection", track);
  server.on("connect", (request, client, head) => {
    const target = new URL("http://" + request.url);
    const remote = track(
      net.connect(Number(target.port) || 443, target.hostname),
    );
    const cleanup = () => {
      client.destroy();
      remote.destroy();
    };
    client.on("error", cleanup);
    remote.on("error", cleanup);
    client.once("close", () => remote.destroy());
    remote.once("close", () => client.destroy());
    remote.once("connect", () => {
      client.write("HTTP/1.1 200 Connection Established\r\n\r\n");
      const upload = up.stream(),
        download = down.stream();
      if (head.length) upload.write(head);
      client.pipe(upload).pipe(remote);
      remote.pipe(download).pipe(client);
      client.once("close", () => {
        upload.destroy();
        download.destroy();
      });
    });
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return {
    server: "http://127.0.0.1:" + server.address().port,
    stats: () => ({
      downloadBytes: down.bytes,
      uploadBytes: up.bytes,
      downloadBytesPerSecond: 200000,
      uploadBytesPerSecond: 93750,
      oneWayDelayMs: 75,
      burstBytes: 16384,
    }),
    close: async () => {
      for (const socket of sockets) socket.destroy();
      down.close();
      up.close();
      await new Promise((resolve) => server.close(resolve));
    },
  };
}
module.exports = { startNetworkProxy };
