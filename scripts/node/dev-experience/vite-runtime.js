const http = require("node:http");
const net = require("node:net");
const fs = require("node:fs");
const path = require("node:path");
const { spawn } = require("node:child_process");
const { getServiceDefinitions } = require("../dev-up/services.js");

function reservePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close((error) => {
        if (error) reject(error);
        else resolve(address.port);
      });
    });
  });
}

function readJsonUrl(url, timeout = 1000) {
  return new Promise((resolve, reject) => {
    const request = http.get(url, { timeout }, (response) => {
      let body = "";
      response.setEncoding("utf8");
      response.on("data", (chunk) => {
        if (body.length < 16_384) body += chunk;
      });
      response.on("end", () => {
        try {
          resolve({ status: response.statusCode, body: JSON.parse(body) });
        } catch (error) {
          reject(error);
        }
      });
    });
    request.once("timeout", () =>
      request.destroy(new Error("request timed out")),
    );
    request.once("error", reject);
  });
}

async function waitForReady(baseUrl, timeout = 90_000) {
  const startedAt = Date.now();
  let latestError = null;
  while (Date.now() - startedAt < timeout) {
    try {
      const receipt = await readJsonUrl(`${baseUrl}/__1flowbase_dev_ready`);
      if (receipt.status === 200 && receipt.body.state === "Ready")
        return receipt.body;
      latestError = new Error(
        `readiness state ${receipt.body.state || "unknown"}`,
      );
    } catch (error) {
      latestError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(
    `isolated Vite cache rebuild did not become ready: ${latestError?.message || "timeout"}`,
  );
}

function createIsolatedViteCacheDirectory(repoRoot) {
  const cacheRoot = path.join(repoRoot, "web", "app", "node_modules");
  fs.mkdirSync(cacheRoot, { recursive: true });
  return fs.mkdtempSync(path.join(cacheRoot, ".vite-dev-experience-cache-"));
}

async function startIsolatedVite(repoRoot) {
  const port = await reservePort();
  const cacheDirectory = createIsolatedViteCacheDirectory(repoRoot);
  const child = spawn(
    "pnpm",
    [
      "--filter",
      "@1flowbase/web",
      "exec",
      "vite",
      "--force",
      "--port",
      String(port),
    ],
    {
      cwd: path.join(repoRoot, "web"),
      env: {
        ...process.env,
        VITE_DEV_SERVER_PORT: String(port),
        VITE_DEV_CACHE_DIR: cacheDirectory,
      },
      detached: process.platform !== "win32",
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  let log = "";
  const capture = (chunk) => {
    if (log.length < 65_536)
      log += chunk.toString().slice(0, 65_536 - log.length);
  };
  child.stdout.on("data", capture);
  child.stderr.on("data", capture);
  const baseUrl = `http://127.0.0.1:${port}`;
  try {
    const readiness = await waitForReady(baseUrl);
    return { baseUrl, cacheDirectory, child, log: () => log, readiness };
  } catch (error) {
    if (process.platform === "win32" || !child.pid) child.kill("SIGTERM");
    else process.kill(-child.pid, "SIGTERM");
    fs.rmSync(cacheDirectory, { recursive: true, force: true });
    throw new Error(`${error.message}\n${log.slice(-4000)}`);
  }
}

async function stopIsolatedVite(runtime) {
  if (runtime.child.exitCode === null) {
    if (process.platform === "win32" || !runtime.child.pid)
      runtime.child.kill("SIGTERM");
    else process.kill(-runtime.child.pid, "SIGTERM");
    await Promise.race([
      new Promise((resolve) => runtime.child.once("exit", resolve)),
      new Promise((resolve) => setTimeout(resolve, 5000)),
    ]);
  }
  const temporaryName = path.basename(runtime.cacheDirectory);
  if (temporaryName.startsWith(".vite-dev-experience-cache-")) {
    fs.rmSync(runtime.cacheDirectory, { recursive: true, force: true });
  }
}

function apiBaseUrlForRepo(repoRoot) {
  const service = getServiceDefinitions(repoRoot)["api-server"];
  return `http://${service.probeHost || "127.0.0.1"}:${service.port}`;
}

module.exports = { createIsolatedViteCacheDirectory, startIsolatedVite, stopIsolatedVite, apiBaseUrlForRepo };
