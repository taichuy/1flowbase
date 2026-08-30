const crypto = require("node:crypto");
const http = require("node:http");
const net = require("node:net");
const fs = require("node:fs");
const path = require("node:path");
const { execFileSync, spawn } = require("node:child_process");

const {
  getRepoRoot,
  resolveOutputDir,
} = require("../testing/warning-capture.js");
const {
  buildChromiumLaunchOptions,
  loadPlaywright,
} = require("../page-debug/core.js");
const {
  loadRootCredentials,
  openTemporaryConsoleSession,
} = require("../page-debug/auth.js");
const { getServiceDefinitions } = require("../dev-up/services.js");

const DEFAULT_MANIFEST_PATH = path.join(__dirname, "manifest.json");
const DEFAULT_WEB_BASE_URL = "http://127.0.0.1:3100";
const SOURCE_EXTENSIONS = new Set([".js", ".jsx", ".ts", ".tsx"]);
const ANT_ICON_BARREL_IMPORT =
  /\bimport\s+(?:\{[^;]*\}|\*\s+as\s+\w+)\s+from\s+['"]@ant-design\/icons['"]/gu;
const IMPORT_SPECIFIER =
  /\b(?:import|export)\s+(?:[\s\S]*?\s+from\s+)?['"]([^'"]+)['"]/gu;
const DYNAMIC_IMPORT = /\bimport\s*\(\s*['"]([^'"]+)['"]\s*\)/gu;
const BROWSER_RESOURCE_CONSOLE_ERROR =
  /^Failed to load resource: the server responded with a status of \d+/u;

function isActionableConsoleError(message) {
  return !BROWSER_RESOURCE_CONSOLE_ERROR.test(message);
}

function normalizePath(value) {
  return value.replace(/\\/gu, "/");
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function listSourceFiles(root) {
  const result = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const absolutePath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      if (!["_tests", "test", "node_modules"].includes(entry.name)) {
        result.push(...listSourceFiles(absolutePath));
      }
    } else if (
      SOURCE_EXTENSIONS.has(path.extname(entry.name)) &&
      !entry.name.endsWith(".d.ts")
    ) {
      result.push(absolutePath);
    }
  }
  return result;
}

function resolveRelativeImport(importer, specifier) {
  if (!specifier.startsWith(".")) {
    return null;
  }
  const base = path.resolve(path.dirname(importer), specifier);
  const candidates = path.extname(base)
    ? [base]
    : [...SOURCE_EXTENSIONS].flatMap((extension) => [
        `${base}${extension}`,
        path.join(base, `index${extension}`),
      ]);
  return candidates.find((candidate) => fs.existsSync(candidate)) || null;
}

function collectSpecifiers(source) {
  const values = [];
  for (const pattern of [IMPORT_SPECIFIER, DYNAMIC_IMPORT]) {
    pattern.lastIndex = 0;
    let match = pattern.exec(source);
    while (match) {
      values.push(match[1]);
      match = pattern.exec(source);
    }
  }
  return [...new Set(values)];
}

function buildSourceGraph(sourceRoot) {
  const files = listSourceFiles(sourceRoot);
  const fileSet = new Set(files);
  const graph = new Map(files.map((filePath) => [filePath, []]));
  for (const filePath of files) {
    const source = fs.readFileSync(filePath, "utf8");
    for (const specifier of collectSpecifiers(source)) {
      const resolved = resolveRelativeImport(filePath, specifier);
      if (resolved && fileSet.has(resolved)) {
        graph.get(filePath).push(resolved);
      }
    }
  }
  return graph;
}

function reachableModules(graph, entryFile) {
  const visited = new Set();
  const pending = [entryFile];
  while (pending.length > 0) {
    const current = pending.pop();
    if (!current || visited.has(current) || !graph.has(current)) continue;
    visited.add(current);
    pending.push(...graph.get(current));
  }
  return [...visited].sort();
}

function routeGraphProfiles(graph, repoRoot) {
  const entries = {
    router: "web/app/src/app/router.tsx",
    frontstage: "web/app/src/features/frontstage/pages/FrontStagePage.tsx",
    settings: "web/app/src/features/settings/pages/SettingsPage.tsx",
    workflow: "web/app/src/features/workflow/pages/WorkflowEditorPage.tsx",
  };
  return Object.fromEntries(
    Object.entries(entries).map(([route, relativePath]) => {
      const modules = reachableModules(
        graph,
        path.join(repoRoot, relativePath),
      );
      return [
        route,
        {
          entry: relativePath,
          reachableModuleCount: modules.length,
          reachableModules: modules.map((filePath) =>
            normalizePath(path.relative(repoRoot, filePath)),
          ),
        },
      ];
    }),
  );
}

function highFanoutOwners(graph, repoRoot, limit = 20) {
  return [...graph.entries()]
    .map(([filePath, dependencies]) => ({
      file: normalizePath(path.relative(repoRoot, filePath)),
      directDependencies: dependencies.length,
    }))
    .sort(
      (left, right) =>
        right.directDependencies - left.directDependencies ||
        left.file.localeCompare(right.file),
    )
    .slice(0, limit);
}

function stronglyConnectedComponents(graph) {
  let nextIndex = 0;
  const indices = new Map();
  const lowLinks = new Map();
  const stack = [];
  const onStack = new Set();
  const components = [];

  function visit(node) {
    indices.set(node, nextIndex);
    lowLinks.set(node, nextIndex);
    nextIndex += 1;
    stack.push(node);
    onStack.add(node);
    for (const neighbor of graph.get(node) || []) {
      if (!indices.has(neighbor)) {
        visit(neighbor);
        lowLinks.set(
          node,
          Math.min(lowLinks.get(node), lowLinks.get(neighbor)),
        );
      } else if (onStack.has(neighbor)) {
        lowLinks.set(node, Math.min(lowLinks.get(node), indices.get(neighbor)));
      }
    }
    if (lowLinks.get(node) === indices.get(node)) {
      const component = [];
      let member;
      do {
        member = stack.pop();
        onStack.delete(member);
        component.push(member);
      } while (member !== node);
      components.push(component.sort());
    }
  }

  for (const node of graph.keys()) {
    if (!indices.has(node)) {
      visit(node);
    }
  }
  return components;
}

function buildCondensation(graph, components) {
  const componentByNode = new Map();
  components.forEach((component, index) => {
    component.forEach((node) => componentByNode.set(node, index));
  });
  const edges = components.map(() => new Set());
  for (const [node, neighbors] of graph.entries()) {
    const from = componentByNode.get(node);
    for (const neighbor of neighbors) {
      const to = componentByNode.get(neighbor);
      if (from !== to) {
        edges[from].add(to);
      }
    }
  }
  return { componentByNode, edges };
}

function reachableComponentSets(edges) {
  const cache = new Map();
  function visit(index) {
    if (cache.has(index)) {
      return cache.get(index);
    }
    const reachable = new Set([index]);
    cache.set(index, reachable);
    for (const neighbor of edges[index]) {
      for (const nested of visit(neighbor)) {
        reachable.add(nested);
      }
    }
    return reachable;
  }
  return edges.map((_edge, index) => visit(index));
}

function median(values) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2
    ? sorted[middle]
    : (sorted[middle - 1] + sorted[middle]) / 2;
}

function robustLimit(history, multiplier = 3) {
  if (history.length < 5) return null;
  const center = median(history);
  const deviation = median(history.map((value) => Math.abs(value - center)));
  return center + multiplier * 1.4826 * deviation;
}

function evaluateBudget({ value, absoluteMax, history = [] }) {
  const regressionMax = robustLimit(history);
  return {
    ok:
      value <= absoluteMax &&
      (regressionMax === null || value <= regressionMax),
    absoluteMax,
    regressionMax,
    historyStatus: regressionMax === null ? "warming" : "active",
  };
}

function cacheIdentity({
  files,
  runtime = process.version,
  platform = process.platform,
  arch = process.arch,
  mode = "development",
  env = {},
}) {
  const hash = crypto.createHash("sha256");
  for (const filePath of [...files].sort()) {
    hash.update(normalizePath(filePath));
    hash.update("\0");
    hash.update(
      fs.existsSync(filePath) ? fs.readFileSync(filePath) : "<missing>",
    );
    hash.update("\0");
  }
  hash.update(JSON.stringify({ runtime, platform, arch, mode, env }));
  return hash.digest("hex");
}

function parseOptimizeDepsInclude(source) {
  const marker = /optimizeDeps\s*:\s*\{/u.exec(source);
  if (!marker) return [];
  const openBrace = source.indexOf("{", marker.index);
  let depth = 0;
  let closeBrace = -1;
  for (let index = openBrace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") depth -= 1;
    if (depth === 0) {
      closeBrace = index;
      break;
    }
  }
  const optimizeDeps =
    closeBrace > openBrace ? source.slice(openBrace + 1, closeBrace) : "";
  const include = /include\s*:\s*\[([\s\S]*?)\]/u.exec(optimizeDeps)?.[1] || "";
  return [...include.matchAll(/['"]([^'"]+)['"]/gu)].map((match) => match[1]);
}

function analyzeStatic({ repoRoot = getRepoRoot() } = {}) {
  const sourceRoot = path.join(repoRoot, "web", "app", "src");
  const graph = buildSourceGraph(sourceRoot);
  const components = stronglyConnectedComponents(graph);
  const condensation = buildCondensation(graph, components);
  const reachable = reachableComponentSets(condensation.edges);
  const findings = [];
  for (const filePath of graph.keys()) {
    const source = fs.readFileSync(filePath, "utf8");
    ANT_ICON_BARREL_IMPORT.lastIndex = 0;
    let match = ANT_ICON_BARREL_IMPORT.exec(source);
    while (match) {
      findings.push({
        code: "high-fanout-icon-barrel",
        file: normalizePath(path.relative(repoRoot, filePath)),
        dependency: "@ant-design/icons",
      });
      match = ANT_ICON_BARREL_IMPORT.exec(source);
    }
  }
  const viteSource = fs.readFileSync(
    path.join(repoRoot, "web", "app", "vite.config.ts"),
    "utf8",
  );
  const optimizedDependencies = new Set(parseOptimizeDepsInclude(viteSource));
  for (const dependency of ["antd", "@ant-design/icons"]) {
    if (!optimizedDependencies.has(dependency)) {
      findings.push({ code: "missing-optimize-dep", dependency });
    }
  }
  return {
    ok: findings.length === 0,
    findings,
    graph: {
      vertices: graph.size,
      edges: [...graph.values()].reduce((sum, edges) => sum + edges.length, 0),
      stronglyConnectedComponents: components.length,
      maxReachableComponents: Math.max(0, ...reachable.map((set) => set.size)),
      routes: routeGraphProfiles(graph, repoRoot),
      highFanoutOwners: highFanoutOwners(graph, repoRoot),
    },
  };
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
          .slice(-3)
          .every(
            (state, index) => state === ["Degraded", "Warming", "Ready"][index],
          )
      ) {
        throw new Error(
          `runtime recovery transition mismatch: ${states.join(" -> ")}`,
        );
      }
      return readiness.body.transitions.slice(-3);
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error("runtime recovery probe timed out before Ready");
}

function profileHistoryKey(scenario, phase, metric) {
  return `${scenario}\0${phase}\0${metric}`;
}

function attachProfileGates(profile, budgets, scenario, history = {}) {
  return {
    ...profile,
    gates: {
      modules: evaluateBudget({
        value: profile.moduleRequestCount || 0,
        absoluteMax: budgets.cold_module_requests_max,
        history:
          history[
            profileHistoryKey(scenario, profile.phase, "moduleRequestCount")
          ] || [],
      }),
      decodedBytes: evaluateBudget({
        value: profile.decodedBytes || 0,
        absoluteMax: budgets.decoded_bytes_max,
        history:
          history[profileHistoryKey(scenario, profile.phase, "decodedBytes")] ||
          [],
      }),
      failures: evaluateBudget({
        value: profile.failedRequests.length,
        absoluteMax: budgets.failed_modules_max,
        history:
          history[
            profileHistoryKey(scenario, profile.phase, "failedRequests")
          ] || [],
      }),
      runtimeErrors: evaluateBudget({
        value:
          (profile.consoleErrors?.length || 0) +
          (profile.pageErrors?.length || 0),
        absoluteMax: budgets.runtime_errors_max,
        history:
          history[
            profileHistoryKey(scenario, profile.phase, "runtimeErrors")
          ] || [],
      }),
      duration: evaluateBudget({
        value: profile.durationMs || 0,
        absoluteMax: budgets.duration_ms_max ?? Number.MAX_SAFE_INTEGER,
        history:
          history[profileHistoryKey(scenario, profile.phase, "durationMs")] ||
          [],
      }),
      pending: evaluateBudget({
        value: profile.pendingRequests?.length || 0,
        absoluteMax: budgets.pending_modules_max ?? 0,
        history:
          history[
            profileHistoryKey(scenario, profile.phase, "pendingRequests")
          ] || [],
      }),
    },
  };
}

function writeReport({ repoRoot, result, env = process.env }) {
  const outputDir = resolveOutputDir(repoRoot, env);
  fs.mkdirSync(outputDir, { recursive: true });
  const reportPath = path.join(outputDir, "dev-experience-profile.json");
  fs.writeFileSync(reportPath, `${JSON.stringify(result, null, 2)}\n`, "utf8");
  return reportPath;
}

function buildReferenceIdentity(
  repoRoot,
  manifestPath,
  { webBaseUrl, publicWebBaseUrl },
) {
  const identityHash = crypto.createHash("sha256");
  for (const filePath of [
    manifestPath,
    path.join(repoRoot, "web", "app", "vite.config.ts"),
  ]) {
    identityHash.update(fs.readFileSync(filePath));
  }
  identityHash.update(
    JSON.stringify({
      node: process.version,
      platform: process.platform,
      arch: process.arch,
      webBaseUrl,
      publicWebBaseUrl,
    }),
  );
  return identityHash.digest("hex");
}

function readHistory({ repoRoot, referenceIdentity, env = process.env }) {
  const reportPath = path.join(
    resolveOutputDir(repoRoot, env),
    "dev-experience-profile.json",
  );
  if (!fs.existsSync(reportPath)) return {};
  try {
    const report = readJson(reportPath);
    return report.referenceIdentity === referenceIdentity
      ? report.history || {}
      : {};
  } catch (_error) {
    return {};
  }
}

function updateHistory(history, profiles) {
  const next = Object.fromEntries(
    Object.entries(history).map(([key, values]) => [key, values.slice(-19)]),
  );
  for (const profile of profiles) {
    const metrics = {
      moduleRequestCount: profile.moduleRequestCount || 0,
      decodedBytes: profile.decodedBytes || 0,
      durationMs: profile.durationMs || 0,
      pendingRequests: profile.pendingRequests?.length || 0,
      failedRequests: profile.failedRequests.length,
      runtimeErrors:
        (profile.consoleErrors?.length || 0) +
        (profile.pageErrors?.length || 0),
    };
    for (const [metric, value] of Object.entries(metrics)) {
      const key = profileHistoryKey(profile.scenario, profile.phase, metric);
      next[key] = [...(next[key] || []), value].slice(-20);
    }
  }
  return next;
}

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

function findFrontstageFixture(nodes, inheritedSlug = null) {
  for (const node of nodes) {
    const slug =
      typeof node.slug === "string" && node.slug.length > 0
        ? node.slug
        : inheritedSlug;
    if (node.kind === "page" && slug) return { pageId: node.id, slug };
    const nested = findFrontstageFixture(
      Array.isArray(node.children) ? node.children : [],
      slug,
    );
    if (nested) return nested;
  }
  return null;
}

async function requestData(requestContext, apiPath) {
  const response = await requestContext.get(apiPath);
  if (!response.ok()) {
    throw new Error(
      `Dev experience fixture request failed: GET ${apiPath} -> ${response.status()} ${await response.text()}`,
    );
  }
  const payload = await response.json();
  return payload?.data;
}

async function resolveScenarioFixtures({
  playwright,
  apiBaseUrl,
  storageStatePath,
  scenarios,
}) {
  const requestContext = await playwright.request.newContext({
    baseURL: apiBaseUrl,
    storageState: storageStatePath,
  });
  try {
    let pages = null;
    let applications = null;
    const resolved = [];
    for (const scenario of scenarios) {
      if (scenario.fixture_kind === "static") {
        resolved.push({ ...scenario, fixture_path: scenario.fixture_path });
        continue;
      }
      if (scenario.fixture_kind === "frontstage_page") {
        pages ||=
          (await requestData(
            requestContext,
            "/api/console/frontstage/pages",
          )) || [];
        const fixture = findFrontstageFixture(pages);
        if (!fixture) {
          throw new Error(
            "Dev experience requires at least one existing Frontstage page fixture",
          );
        }
        resolved.push({
          ...scenario,
          fixture_path: scenario.path_template
            .replace(":slug", fixture.slug)
            .replace(":page_id", fixture.pageId),
        });
        continue;
      }
      if (scenario.fixture_kind === "workflow_application") {
        applications ||=
          (await requestData(requestContext, "/api/console/applications")) ||
          [];
        const application = applications.find(
          (candidate) => candidate.application_type === "workflow",
        );
        if (!application) {
          throw new Error(
            "Dev experience requires at least one existing workflow application fixture",
          );
        }
        resolved.push({
          ...scenario,
          fixture_path: scenario.path_template.replace(
            ":application_id",
            application.id,
          ),
        });
        continue;
      }
      throw new Error(
        `Unknown dev experience fixture kind: ${scenario.fixture_kind}`,
      );
    }
    return resolved;
  } finally {
    await requestContext.dispose();
  }
}

async function runSmoke({
  repoRoot = getRepoRoot(),
  manifestPath = DEFAULT_MANIFEST_PATH,
  webBaseUrl = DEFAULT_WEB_BASE_URL,
  publicWebBaseUrl = null,
  env = process.env,
} = {}) {
  const manifest = readJson(manifestPath);
  const playwright = loadPlaywright(repoRoot);
  const outputDir = resolveOutputDir(repoRoot, env);
  fs.mkdirSync(outputDir, { recursive: true });
  const storageStatePath = path.join(
    outputDir,
    "dev-experience-storage-state.json",
  );
  const apiBaseUrl = apiBaseUrlForRepo(repoRoot);
  const credentials = loadRootCredentials({ repoRoot, sourceEnv: env });
  const temporarySession = await openTemporaryConsoleSession({
    playwright,
    apiBaseUrl,
    account: credentials.account,
    password: credentials.password,
    storageStatePath,
  });
  let scenarios;
  let browser;
  try {
    scenarios = await resolveScenarioFixtures({
      playwright,
      apiBaseUrl,
      storageStatePath,
      scenarios: manifest.scenarios,
    });
    browser = await playwright.chromium.launch(
      buildChromiumLaunchOptions({ headless: true, env }),
    );
  } catch (error) {
    await temporarySession.dispose();
    if (fs.existsSync(storageStatePath)) fs.unlinkSync(storageStatePath);
    throw error;
  }
  const profiles = [];
  const referenceIdentity = buildReferenceIdentity(repoRoot, manifestPath, {
    webBaseUrl,
    publicWebBaseUrl,
  });
  const history = readHistory({ repoRoot, referenceIdentity, env });
  try {
    for (const scenario of scenarios) {
      const scenarioBaseUrl =
        scenario.base_url_kind === "public" ? publicWebBaseUrl : webBaseUrl;
      if (!scenarioBaseUrl) {
        throw new Error(
          `Dev experience scenario '${scenario.id}' requires --public-web-base-url`,
        );
      }
      const targetUrl = `${scenarioBaseUrl.replace(/\/$/u, "")}${scenario.fixture_path}`;
      const context = await browser.newContext(
        scenario.session_kind === "incognito"
          ? {}
          : { storageState: storageStatePath },
      );
      const page = await context.newPage();
      const cold = await profileScenario({
        page,
        phase: "cold",
        url: targetUrl,
        scenario,
      });
      profiles.push({
        scenario: scenario.id,
        ...attachProfileGates(cold, scenario.budgets, scenario.id, history),
      });
      const warm = await profileScenario({
        page,
        phase: "warm",
        url: targetUrl,
        scenario,
      });
      profiles.push({
        scenario: scenario.id,
        ...attachProfileGates(warm, scenario.budgets, scenario.id, history),
      });
      if (scenario.phases.includes("hmr")) {
        const hmr = await profileHmrTransport({ page, webBaseUrl });
        profiles.push({
          scenario: scenario.id,
          ...attachProfileGates(hmr, scenario.budgets, scenario.id, history),
        });
      }
      if (scenario.phases.includes("recovery")) {
        const recoveryTransitions = await triggerRuntimeRecovery({
          page,
          webBaseUrl,
        });
        const recovery = await profileScenario({
          page,
          phase: "recovery",
          url: targetUrl,
          scenario,
        });
        profiles.push({
          scenario: scenario.id,
          recoveryTransitions,
          ...attachProfileGates(
            recovery,
            scenario.budgets,
            scenario.id,
            history,
          ),
        });
      }
      await context.close();

      if (scenario.phases.includes("concurrent")) {
        const contexts = await Promise.all(
          Array.from({ length: 4 }, () =>
            browser.newContext({ storageState: storageStatePath }),
          ),
        );
        const concurrentProfiles = await Promise.all(
          contexts.map(async (candidateContext) => {
            const candidatePage = await candidateContext.newPage();
            return profileScenario({
              page: candidatePage,
              phase: "concurrent",
              url: targetUrl,
              scenario,
            });
          }),
        );
        profiles.push({
          scenario: scenario.id,
          ...attachProfileGates(
            {
              phase: "concurrent",
              durationMs: Math.max(
                ...concurrentProfiles.map((profile) => profile.durationMs),
              ),
              moduleRequestCount: Math.max(
                ...concurrentProfiles.map(
                  (profile) => profile.moduleRequestCount,
                ),
              ),
              decodedBytes: Math.max(
                ...concurrentProfiles.map((profile) => profile.decodedBytes),
              ),
              failedRequests: concurrentProfiles.flatMap(
                (profile) => profile.failedRequests,
              ),
              consoleErrors: concurrentProfiles.flatMap(
                (profile) => profile.consoleErrors,
              ),
              pageErrors: concurrentProfiles.flatMap(
                (profile) => profile.pageErrors,
              ),
              requestCount: Math.max(
                ...concurrentProfiles.map((profile) => profile.requestCount),
              ),
              transferBytes: Math.max(
                ...concurrentProfiles.map((profile) => profile.transferBytes),
              ),
              transformDurationMs: Math.max(
                ...concurrentProfiles.map(
                  (profile) => profile.transformDurationMs,
                ),
              ),
              finalUrls: concurrentProfiles.map((profile) => profile.finalUrl),
              resources: concurrentProfiles.map((profile) => profile.resources),
            },
            scenario.budgets,
            scenario.id,
            history,
          ),
        });
        await Promise.all(
          contexts.map((candidateContext) => candidateContext.close()),
        );
      }
    }

    if (
      scenarios.some((scenario) => scenario.phases.includes("cache-rebuild"))
    ) {
      const isolated = await startIsolatedVite(repoRoot);
      try {
        for (const scenario of scenarios.filter((candidate) =>
          candidate.phases.includes("cache-rebuild"),
        )) {
          const context = await browser.newContext({
            storageState: storageStatePath,
          });
          const page = await context.newPage();
          let profile;
          try {
            profile = await profileScenario({
              page,
              phase: "cache-rebuild",
              url: `${isolated.baseUrl}${scenario.fixture_path}`,
              scenario,
            });
          } catch (error) {
            throw new Error(
              `${error.message}\n${isolated.log().slice(-4000)}`,
              { cause: error },
            );
          }
          profiles.push({
            scenario: scenario.id,
            cacheIdentity: isolated.readiness.cacheIdentity,
            ...attachProfileGates(
              profile,
              scenario.budgets,
              scenario.id,
              history,
            ),
          });
          await context.close();
        }
      } finally {
        await stopIsolatedVite(isolated);
      }
    }
  } finally {
    try {
      await browser.close();
    } finally {
      await temporarySession.dispose();
      if (fs.existsSync(storageStatePath)) fs.unlinkSync(storageStatePath);
    }
  }
  const staticResult = analyzeStatic({ repoRoot });
  const result = {
    schemaVersion: manifest.schema_version,
    candidateSha: execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: repoRoot,
      encoding: "utf8",
    }).trim(),
    generatedAt: new Date().toISOString(),
    environment: {
      node: process.version,
      platform: process.platform,
      arch: process.arch,
      browser: "chromium",
      webBaseUrl,
      publicWebBaseUrl,
      apiBaseUrl,
    },
    referenceIdentity,
    staticGraph: staticResult.graph,
    ok:
      staticResult.ok &&
      profiles.every((profile) =>
        Object.values(profile.gates).every((gate) => gate.ok),
      ),
    profiles,
    history: updateHistory(history, profiles),
  };
  writeReport({ repoRoot, result, env });
  return result;
}

function parseCliArgs(argv) {
  const options = {
    smoke: false,
    webBaseUrl: DEFAULT_WEB_BASE_URL,
    publicWebBaseUrl: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--smoke") options.smoke = true;
    else if (argv[index] === "--web-base-url" && argv[index + 1])
      options.webBaseUrl = argv[++index];
    else if (argv[index] === "--public-web-base-url" && argv[index + 1])
      options.publicWebBaseUrl = argv[++index];
    else if (argv[index] === "--help" || argv[index] === "-h")
      options.help = true;
    else throw new Error(`Unknown dev-experience option: ${argv[index]}`);
  }
  return options;
}

async function main(argv = [], deps = {}) {
  const options = parseCliArgs(argv);
  const writeStdout =
    deps.writeStdout || ((text) => process.stdout.write(text));
  if (options.help) {
    writeStdout(
      "Usage: node scripts/node/dev-experience.js [--smoke] [--web-base-url <url>] [--public-web-base-url <url>]\n",
    );
    return 0;
  }
  const staticResult = analyzeStatic({
    repoRoot: deps.repoRoot || getRepoRoot(),
  });
  writeStdout(
    `[dev-experience] static ${staticResult.ok ? "passed" : "failed"}: ${staticResult.graph.vertices} modules, ${staticResult.graph.edges} edges\n`,
  );
  if (!staticResult.ok) return 1;
  if (!options.smoke) return 0;
  const smokeResult = await (deps.runSmokeImpl || runSmoke)({
    repoRoot: deps.repoRoot || getRepoRoot(),
    webBaseUrl: options.webBaseUrl,
    publicWebBaseUrl: options.publicWebBaseUrl,
    env: deps.env || process.env,
  });
  return smokeResult.ok ? 0 : 1;
}

module.exports = {
  analyzeStatic,
  buildCondensation,
  buildSourceGraph,
  cacheIdentity,
  createIsolatedViteCacheDirectory,
  evaluateBudget,
  isActionableConsoleError,
  main,
  median,
  parseOptimizeDepsInclude,
  profileScenario,
  profileHmrTransport,
  reachableComponentSets,
  readHistory,
  robustLimit,
  resolveScenarioFixtures,
  runSmoke,
  startIsolatedVite,
  stopIsolatedVite,
  stronglyConnectedComponents,
  summarizeResources,
  triggerRuntimeRecovery,
  updateHistory,
};
