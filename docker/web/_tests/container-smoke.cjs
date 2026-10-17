const http = require("node:http");
const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");
const assert = require("node:assert/strict");
const { execFileSync, spawnSync } = require("node:child_process");
const { createHash } = require("node:crypto");
const image = process.env.FLOWBASE_NGINX_TEST_IMAGE;
assert.ok(
  image,
  "Set FLOWBASE_NGINX_TEST_IMAGE to a locally built runtime-base image",
);
const docker = (args) => {
  if (args[0] === "logs") {
    const result = spawnSync("docker", args, {
      encoding: "utf8",
      timeout: 30000,
    });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout + result.stderr;
  }
  return execFileSync("docker", args, {
    encoding: "utf8",
    timeout: 30000,
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
};
(async () => {
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), "flowbase-nginx-container-"),
  );
  const config = path.join(root, "config");
  fs.mkdirSync(config);
  const upstream = http.createServer((req, res) => {
    res.writeHead(req.url === "/health" ? 200 : 426);
    res.end("fixture");
  });
  upstream.on("upgrade", (req, socket) => {
    if (
      req.headers.host !== `127.0.0.1:${port}` ||
      req.headers["x-forwarded-proto"] !== "https"
    ) {
      socket.end(
        "HTTP/1.1 400 Bad Request\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
      );
      return;
    }
    const accept = createHash("sha1")
      .update(
        req.headers["sec-websocket-key"] +
          "258EAFA5-E914-47DA-95CA-C5AB0DC85B11",
      )
      .digest("base64");
    socket.end(
      `HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`,
    );
  });
  await new Promise((r) => upstream.listen(0, "127.0.0.1", r));
  const reserve = http.createServer();
  await new Promise((r) => reserve.listen(0, "127.0.0.1", r));
  const port = reserve.address().port;
  await new Promise((r) => reserve.close(r));
  const template = path.join(root, "template.conf");
  const source = fs
    .readFileSync("docker/web/nginx.conf", "utf8")
    .replace("listen 80;", `listen 127.0.0.1:${port};`)
    .replaceAll("api:7800", `127.0.0.1:${upstream.address().port}`);
  fs.writeFileSync(template, source);
  const name = `flowbase-nginx-lifecycle-${process.pid}`;
  const remove = () => {
    try {
      docker(["rm", "-f", name]);
    } catch {}
  };
  const probe = (requestPath) =>
    new Promise((resolve, reject) => {
      const request = http.request(
        {
          host: "127.0.0.1",
          port,
          path: requestPath,
          headers: {
            "X-Forwarded-Proto": "https",
            Connection: "Upgrade",
            Upgrade: "websocket",
            "Sec-WebSocket-Version": "13",
            "Sec-WebSocket-Key": "dGhlIHNhbXBsZSBub25jZQ==",
          },
        },
        (res) => {
          res.resume();
          resolve(res.statusCode);
        },
      );
      request.on("upgrade", (res, socket) => {
        socket.destroy();
        resolve(res.statusCode);
      });
      request.on("error", reject);
      request.setTimeout(2000, () => request.destroy(new Error("timeout")));
      request.end();
    });
  const start = async (extra = [], expectReady = true) => {
    docker([
      "run",
      "-d",
      "--pull=never",
      "--name",
      name,
      "--network",
      "host",
      "-v",
      `${config}:/config/nginx`,
      "-v",
      `${template}:/usr/local/share/1flowbase/nginx.conf:ro`,
      ...extra,
      image,
    ]);
    if (!expectReady) {
      assert.notEqual(docker(["wait", name]), "0");
      return;
    }
    for (let i = 0; i < 100; i++) {
      try {
        assert.equal(
          await probe(
            "/api/console/managed-services/ssh/hosts/fixture/terminal/stream",
          ),
          101,
        );
        assert.equal(await probe("/v1/fixture"), 101);
        return;
      } catch (error) {
        if (i === 99) throw error;
        await new Promise((r) => setTimeout(r, 100));
      }
    }
  };
  const results = [];
  try {
    await start();
    assert.equal(
      fs.readFileSync(path.join(config, "nginx.default.conf"), "utf8"),
      source,
    );
    assert.equal(fs.existsSync(path.join(config, "nginx.conf")), false);
    results.push("empty directory: default exported, WebSocket 101");
    remove();
    fs.writeFileSync(template, source + "\n# image upgrade\n");
    await start();
    assert.match(
      fs.readFileSync(path.join(config, "nginx.default.conf"), "utf8"),
      /image upgrade/,
    );
    results.push("recreation: new image default applied, WebSocket 101");
    remove();
    const custom = path.join(config, "nginx.conf");
    fs.writeFileSync(custom, source + "\n# operator override\n");
    await start();
    assert.match(
      docker(["exec", name, "cat", "/etc/nginx/conf.d/default.conf"]),
      /operator override/,
    );
    assert.match(fs.readFileSync(custom, "utf8"), /operator override/);
    results.push("custom override: preserved and active, WebSocket 101");
    remove();
    fs.unlinkSync(custom);
    await start();
    assert.match(
      docker(["exec", name, "cat", "/etc/nginx/conf.d/default.conf"]),
      /image upgrade/,
    );
    results.push("override removed: current default restored, WebSocket 101");
    remove();
    const legacy = path.join(root, "legacy.conf");
    fs.writeFileSync(legacy, source + "\n# legacy mount\n");
    await start(["-v", `${legacy}:/etc/nginx/conf.d/default.conf:ro`]);
    assert.match(
      docker(["exec", name, "cat", "/etc/nginx/conf.d/default.conf"]),
      /legacy mount/,
    );
    assert.match(docker(["logs", name]), /retaining legacy mounted/);
    results.push("legacy read-only file mount: preserved, WebSocket 101");
    remove();
    fs.mkdirSync(custom);
    await start([], false);
    assert.match(docker(["logs", name]), /must be a readable file/);
    results.push("incorrect directory: clear startup failure");
    remove();
    fs.rmdirSync(custom);
    fs.writeFileSync(custom, "invalid nginx syntax;");
    await start([], false);
    assert.match(docker(["logs", name]), /using persistent override/);
    results.push("invalid override: nginx validation stops startup");
    remove();
    console.log(JSON.stringify(results, null, 2));
  } finally {
    remove();
    upstream.closeAllConnections();
    await new Promise((r) => upstream.close(r));
    // Generated templates are written by container root.
    docker([
      "run",
      "--rm",
      "--pull=never",
      "--entrypoint",
      "sh",
      "-v",
      `${root}:/fixture`,
      image,
      "-c",
      "rm -rf /fixture/config",
    ]);
    fs.rmSync(root, { recursive: true, force: true });
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
