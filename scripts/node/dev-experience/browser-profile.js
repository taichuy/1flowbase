

const BROWSER_RESOURCE_CONSOLE_ERROR =
  /^Failed to load resource: the server responded with a status of \d+/u;

function isActionableConsoleError(message) {
  return !BROWSER_RESOURCE_CONSOLE_ERROR.test(message);
}

function classifyResource(entry) {
  const url = entry.name || "";
  if (/\/(?:src|@fs)\/|\.(?:[cm]?[jt]sx?)(?:\?|$)/u.test(url)) return "module";
  if (/node_modules|\.vite\/deps/u.test(url)) return "dependency";
  return "asset";
}

function summarizeResources(entries, failedRequests = []) {
  const resources = entries.map((entry) => ({
    url: entry.name,
    category: classifyResource(entry),
    transferSize: entry.transferSize || 0,
    encodedBodySize: entry.encodedBodySize || 0,
    decodedBodySize: entry.decodedBodySize || 0,
    duration: entry.duration || 0,
    initiatorType: entry.initiatorType || "other",
  }));
  return {
    requestCount: resources.length,
    moduleRequestCount: resources.filter(
      (resource) => resource.category === "module",
    ).length,
    transferBytes: resources.reduce(
      (sum, resource) => sum + resource.transferSize,
      0,
    ),
    decodedBytes: resources.reduce(
      (sum, resource) => sum + resource.decodedBodySize,
      0,
    ),
    transformDurationMs: resources
      .filter((resource) => resource.category === "module")
      .reduce((sum, resource) => sum + resource.duration, 0),
    failedRequests,
    resources,
  };
}

async function profileScenario({
  page,
  url,
  phase,
  scenario,
  timeout = 60_000,
}) {
  const failedRequests = [];
  const consoleErrors = [];
  const pageErrors = [];
  const pendingRequests = new Set();
  const onRequest = (request) => pendingRequests.add(request.url());
  const onRequestFinished = (request) => pendingRequests.delete(request.url());
  const onRequestFailed = (request) => {
    pendingRequests.delete(request.url());
    failedRequests.push({
      url: request.url(),
      error: request.failure()?.errorText || "unknown",
    });
  };
  const onResponse = (response) => {
    const urlValue = response.url();
    if (
      response.status() >= 400 &&
      classifyResource({ name: urlValue }) === "module"
    ) {
      failedRequests.push({
        url: urlValue,
        error: `HTTP ${response.status()}`,
      });
    }
  };
  const onConsole = (message) => {
    if (
      message.type() === "error" &&
      isActionableConsoleError(message.text())
    ) {
      consoleErrors.push(message.text());
    }
  };
  const onPageError = (error) =>
    pageErrors.push(error.message || String(error));
  page.on("request", onRequest);
  page.on("requestfinished", onRequestFinished);
  page.on("requestfailed", onRequestFailed);
  page.on("response", onResponse);
  page.on("console", onConsole);
  page.on("pageerror", onPageError);
  try {
    const startedAt = Date.now();
    await page.goto(url, { waitUntil: "domcontentloaded", timeout });
    await page.waitForSelector(scenario.ready_selector, {
      state: "visible",
      timeout,
    });
    const readyDurationMs = Date.now() - startedAt;
    await page
      .waitForLoadState("networkidle", { timeout: Math.min(timeout, 5_000) })
      .catch(() => {});
    const entries = await page.evaluate(() =>
      performance.getEntriesByType("resource").map((entry) => ({
        name: entry.name,
        initiatorType: entry.initiatorType,
        transferSize: entry.transferSize,
        encodedBodySize: entry.encodedBodySize,
        decodedBodySize: entry.decodedBodySize,
        duration: entry.duration,
      })),
    );
    const finalUrl = page.url();
    const finalPath = new URL(finalUrl).pathname;
    const expectedPathMatches = scenario.expected_path_pattern
      ? new RegExp(scenario.expected_path_pattern, "u").test(finalPath)
      : finalPath.startsWith(scenario.expected_path_prefix);
    if (!expectedPathMatches) {
      pageErrors.push(
        `unexpected final path ${finalPath}; expected ${scenario.expected_path_pattern || scenario.expected_path_prefix}`,
      );
    }
    return {
      phase,
      readyDurationMs,
      durationMs: Date.now() - startedAt,
      finalUrl,
      consoleErrors,
      pageErrors,
      pendingRequests: [...pendingRequests].filter(
        (urlValue) => classifyResource({ name: urlValue }) === "module",
      ),
      ...summarizeResources(entries, failedRequests),
    };
  } finally {
    page.off("request", onRequest);
    page.off("requestfinished", onRequestFinished);
    page.off("requestfailed", onRequestFailed);
    page.off("response", onResponse);
    page.off("console", onConsole);
    page.off("pageerror", onPageError);
  }
}

async function profileHmrTransport({ page, webBaseUrl, timeout = 10_000 }) {
  await page.evaluate(() => {
    delete globalThis.__ONEFLOWBASE_DEV_HMR_RECEIPT__;
  });
  const requestStartedAt = Date.now();
  const response = await page.evaluate(
    async (probeUrl) => {
      const result = await fetch(probeUrl, { cache: "no-store" });
      return result.json();
    },
    `${webBaseUrl.replace(/\/$/u, "")}/__1flowbase_dev_hmr_probe`,
  );
  await page.waitForFunction(
    (token) => globalThis.__ONEFLOWBASE_DEV_HMR_RECEIPT__?.token === token,
    response.token,
    { timeout },
  );
  const receipt = await page.evaluate(
    () => globalThis.__ONEFLOWBASE_DEV_HMR_RECEIPT__,
  );
  return {
    phase: "hmr",
    durationMs: receipt.receivedAt - response.sentAt,
    roundTripMs: Date.now() - requestStartedAt,
    failedRequests: [],
    consoleErrors: [],
    pageErrors: [],
  };
}

async function triggerRuntimeRecovery({ page, webBaseUrl, timeout = 30_000 }) {
  const baseUrl = webBaseUrl.replace(/\/$/u, "");
  const degraded = await page.evaluate(async (probeUrl) => {
    const response = await fetch(probeUrl, {
      method: "POST",
      cache: "no-store",
    });
    return { status: response.status, body: await response.json() };
  }, `${baseUrl}/__1flowbase_dev_recovery_probe`);
  if (degraded.status !== 202 || degraded.body.state !== "Degraded") {
    throw new Error(
      `runtime recovery probe did not enter Degraded: ${JSON.stringify(degraded)}`,
    );
  }
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeout) {
    const readiness = await page.evaluate(async (readyUrl) => {
      const response = await fetch(readyUrl, { cache: "no-store" });
      return { status: response.status, body: await response.json() };
    }, `${baseUrl}/__1flowbase_dev_ready`);
    if (readiness.status === 200 && readiness.body.state === "Ready") {
      const states = readiness.body.transitions.map((entry) => entry.state);
      if (
        !states
          .slice(-4)
          .every(
            (state, index) =>
              state === ["Degraded", "Building", "Validating", "Ready"][index],
          )
      ) {
        throw new Error(
          `runtime recovery transition mismatch: ${states.join(" -> ")}`,
        );
      }
      return readiness.body.transitions.slice(-4);
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error("runtime recovery probe timed out before Ready");
}

module.exports = { isActionableConsoleError, summarizeResources, profileScenario, profileHmrTransport, triggerRuntimeRecovery };
