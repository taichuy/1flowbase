#!/usr/bin/env node

// Anonymous public sign-in loading probe: never submits a form or creates a session.
const fs = require("node:fs");
const { createRequire } = require("node:module");
const path = require("node:path");
const repoRoot = path.resolve(__dirname, "../../..");
const { chromium } = createRequire(repoRoot + "/web/package.json")(
  "playwright",
);
(async () => {
  const [label, base, profile = "limited"] = process.argv.slice(2);
  if (label === "--help") {
    console.log(
      "Usage: node scripts/node/page-debug/public-auth-performance.js <label> <origin> [limited|normal]",
    );
    return;
  }
  if (
    !/^[a-zA-Z0-9_-]+$/.test(label ?? "") ||
    !["limited", "normal"].includes(profile)
  )
    throw new Error(
      "Provide a run label, HTTP(S) origin and limited|normal profile.",
    );
  const origin = new URL(base);
  if (!["http:", "https:"].includes(origin.protocol) || origin.pathname !== "/")
    throw new Error("Expected an HTTP(S) origin without a path.");
  const dir = repoRoot + "/tmp/test-governance/auth-loading/" + label;
  fs.mkdirSync(dir, { recursive: true });
  // Wait outside the measured browser so a dev-server restart's 503 response
  // cannot masquerade as a 120-second cold-page sample.
  let stableResponses = 0;
  const readinessDeadline = Date.now() + 90000;
  while (stableResponses < 3 && Date.now() < readinessDeadline) {
    try {
      const response = await fetch(origin.origin + "/", {
        signal: AbortSignal.timeout(5000),
      });
      await response.arrayBuffer();
      stableResponses = response.ok ? stableResponses + 1 : 0;
    } catch {
      stableResponses = 0;
    }
    if (stableResponses < 3)
      await new Promise((resolve) => setTimeout(resolve, 1000));
  }
  if (stableResponses < 3)
    throw new Error("Origin did not become ready within 90 seconds.");
  const proxy =
    profile === "limited"
      ? await require("./network-profile.js").startNetworkProxy()
      : null;
  let browser;
  try {
    browser = await chromium.launch({
      headless: true,
      ...(proxy
        ? {
            proxy: { server: proxy.server, bypass: "<-loopback>" },
            args: ["--proxy-bypass-list=<-loopback>"],
          }
        : {}),
      ...(process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH
        ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH }
        : {}),
    });
  } catch (error) {
    await proxy?.close();
    throw error;
  }
  const records = [],
    errors = [],
    phases = [];
  let active = 0,
    peak = 0,
    clickMs = null,
    inputMs = null,
    selected = null;
  const started = Date.now();
  try {
    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      serviceWorkers: "block",
    });
    await context.addInitScript(() => {
      window.__authCompilerTimings = [];
      const BrowserWorker = window.Worker;
      window.Worker = class extends BrowserWorker {
        constructor(url, options) {
          super(url, options);
          if (options?.name !== "native-react-component-compiler") return;
          const record = {
            start: performance.now(),
            end: null,
            outcome: "pending",
          };
          window.__authCompilerTimings.push(record);
          this.addEventListener("message", (event) => {
            record.end = performance.now();
            record.outcome = event.data?.type;
          });
          this.addEventListener("error", () => {
            record.end = performance.now();
            record.outcome = "error";
          });
        }
      };
    });
    const page = await context.newPage();
    const cdp = await context.newCDPSession(page);
    await cdp.send("Network.enable");

    const pending = new Map();
    cdp.on("Network.requestWillBeSent", (e) => {
      if (!/^https?:/.test(e.request.url)) return;
      const r = {
        url: e.request.url,
        type: e.type,
        startMs: Date.now() - started,
        initiator: e.initiator.type,
      };
      records.push(r);
      pending.set(e.requestId, r);
      active++;
      peak = Math.max(peak, active);
    });
    cdp.on("Network.responseReceived", (e) => {
      const r = pending.get(e.requestId);
      if (r)
        Object.assign(r, {
          status: e.response.status,
          protocol: e.response.protocol,
          cache: e.response.fromDiskCache,
        });
    });
    const finish = (e) => {
      const r = pending.get(e.requestId);
      if (!r) return;
      Object.assign(r, {
        endMs: Date.now() - started,
        bytes: e.encodedDataLength || 0,
        error: e.errorText,
      });
      pending.delete(e.requestId);
      active--;
    };
    cdp.on("Network.loadingFinished", finish);
    cdp.on("Network.loadingFailed", finish);
    const workerNetwork = [],
      workerSetupErrors = [];
    let workerCommand = 0;
    cdp.on("Target.attachedToTarget", async (event) => {
      const send = (method, params = {}) =>
        cdp.send("Target.sendMessageToTarget", {
          sessionId: event.sessionId,
          message: JSON.stringify({ id: ++workerCommand, method, params }),
        });
      try {
        await send("Network.enable");
      } catch (error) {
        workerSetupErrors.push(String(error));
      } finally {
        await send("Runtime.runIfWaitingForDebugger").catch((error) =>
          workerSetupErrors.push(String(error)),
        );
      }
    });
    cdp.on("Target.receivedMessageFromTarget", (event) => {
      const message = JSON.parse(event.message);
      if (message.error) workerSetupErrors.push(message.error);
      if (message.method?.startsWith("Network."))
        workerNetwork.push({
          sessionId: event.sessionId,
          ms: Date.now() - started,
          ...message,
        });
    });
    await cdp.send("Target.setAutoAttach", {
      autoAttach: true,
      waitForDebuggerOnStart: true,
      flatten: false,
      filter: [{ type: "worker" }, { exclude: true }],
    });
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("console", (m) => {
      if (m.type() === "error") errors.push(m.text().slice(0, 500));
    });
    await page.goto(base + "/sign-in", {
      waitUntil: "domcontentloaded",
      timeout: 60000,
    });
    const until = Date.now() + 120000;
    let lastPhase = "";
    while (Date.now() < until) {
      const selectors = page.locator(".auth-sign-in-selector-button");
      if (clickMs === null && (await selectors.count())) {
        selected = await selectors.first().innerText();
        clickMs = Date.now() - started;
        await selectors.first().click({ timeout: 5000 });
      }
      const phase = await page
        .locator("[data-public-auth-phase]")
        .getAttribute("data-public-auth-phase", { timeout: 250 })
        .catch(() => null);
      if (phase && phase !== lastPhase) {
        phases.push({ phase, ms: Date.now() - started });
        lastPhase = phase;
      }
      const input = page.locator("input:not([type=hidden])").first();
      if (await input.isVisible().catch(() => false)) {
        await input.fill("loading-probe");
        inputMs = Date.now() - started;
        break;
      }
      await page.waitForTimeout(200);
    }
    await page.screenshot({ path: dir + "/page.png" });
    const fallback = await page
      .locator('[data-testid="builtin-password-sign-in"]')
      .count();
    const text = await page.locator("body").innerText();
    const compilerTimings = await page.evaluate(
      () => window.__authCompilerTimings,
    );
    const firstLoad = {
      requests: records.length,
      bytes: records.reduce((n, r) => n + (r.bytes || 0), 0),
      proxy: proxy?.stats(),
      workerRequests: workerNetwork.filter(
        (r) => r.method === "Network.requestWillBeSent",
      ).length,
    };
    let revisitMs = null,
      revisitWorkers = null;
    if (inputMs !== null && selected) {
      const back = page
        .getByRole("button", { name: /Back to other sign-in options/ })
        .first();
      if (await back.isVisible().catch(() => false)) {
        await back.click();
        const option = page
          .locator(".auth-sign-in-selector-button")
          .filter({ hasText: selected })
          .first();
        await option.waitFor({ timeout: 10000 });
        const revisiting = Date.now();
        await option.click();
        await page
          .locator("input:not([type=hidden])")
          .first()
          .waitFor({ state: "visible", timeout: 20000 });
        revisitMs = Date.now() - revisiting;
        revisitWorkers =
          (await page.evaluate(() => window.__authCompilerTimings.length)) -
          compilerTimings.length;
      }
    }
    const summary = {
      label,
      base,
      profile,
      firstLoad,
      workerSetupErrors,
      compilerTimings,
      revisitMs,
      revisitWorkers,
      startedAt: new Date(started).toISOString(),
      totalMs: Date.now() - started,
      clickMs,
      inputMs,
      selected,
      phases,
      fallback,
      peak,
      requests: records.length,
      bytes: records.reduce((n, r) => n + (r.bytes || 0), 0),
      failures: records.filter((r) => r.error || r.status >= 400),
      errors,
      text: text.slice(0, 2000),
    };
    fs.writeFileSync(dir + "/summary.json", JSON.stringify(summary, null, 2));
    fs.writeFileSync(dir + "/network.json", JSON.stringify(records, null, 2));
    fs.writeFileSync(
      dir + "/worker-network.json",
      JSON.stringify(workerNetwork, null, 2),
    );
    console.log(JSON.stringify(summary));
    if (inputMs === null || fallback > 0) process.exitCode = 1;
  } finally {
    await browser.close();
    await proxy?.close();
  }
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
