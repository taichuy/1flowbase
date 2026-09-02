const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

const {
  attachProfileGates,
  cacheIdentity,
  createIsolatedViteCacheDirectory,
  evaluateBudget,
  isActionableConsoleError,
  parseOptimizeDepsInclude,
  reachableComponentSets,
  robustLimit,
  resolveScenarioFixtures,
  stronglyConnectedComponents,
  summarizeResources,
  updateHistory,
} = require("../core.js");

test("DV-F01 readiness SLO excludes post-ready network settling", () => {
  const profile = attachProfileGates(
    {
      phase: "cold",
      readyDurationMs: 7_000,
      durationMs: 9_000,
      moduleRequestCount: 1,
      decodedBytes: 1,
      failedRequests: [],
      consoleErrors: [],
      pageErrors: [],
      pendingRequests: [],
    },
    {
      cold_module_requests_max: 10,
      decoded_bytes_max: 10,
      duration_ms_max: 8_000,
      failed_modules_max: 0,
      runtime_errors_max: 0,
    },
    "public-incognito-root",
  );

  assert.equal(profile.gates.duration.ok, true);
});

test("DV-F01 runtime errors exclude Chromium resource status noise", () => {
  assert.equal(
    isActionableConsoleError(
      "Failed to load resource: the server responded with a status of 401 ()",
    ),
    false,
  );
  assert.equal(
    isActionableConsoleError("Failed to fetch dynamically imported module"),
    true,
  );
  assert.equal(isActionableConsoleError("application runtime failed"), true);
});

test("DV-F01 isolated fresh cache stays inside the app module resolution domain", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "dev-experience-root-"));
  const cacheDirectory = createIsolatedViteCacheDirectory(root);
  try {
    assert.equal(
      path.dirname(cacheDirectory),
      path.join(root, "web", "app", "node_modules"),
    );
    assert.match(
      path.basename(cacheDirectory),
      /^\.vite-dev-experience-cache-/u,
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("DV-F01 optimize dependency fixture rejects names outside optimizeDeps include", () => {
  const source = `
    const label = 'antd-vendor';
    export default { optimizeDeps: { include: ['react'] } };
  `;
  assert.deepEqual(parseOptimizeDepsInclude(source), ["react"]);
});

test("DV-F01 runtime fixtures resolve real page and workflow identities", async () => {
  const responses = {
    "/api/console/frontstage/pages": {
      data: [
        {
          id: "group-1",
          kind: "group",
          children: [
            {
              id: "page-1",
              kind: "page",
              slug: "task-planning-ft",
              children: [],
            },
          ],
        },
      ],
    },
    "/api/console/applications": {
      data: [
        { id: "agent-1", application_type: "agent_flow" },
        { id: "workflow-1", application_type: "workflow" },
      ],
    },
  };
  const scenarios = await resolveScenarioFixtures({
    playwright: {
      request: {
        async newContext() {
          return {
            async get(apiPath) {
              return {
                ok: () => true,
                status: () => 200,
                json: async () => responses[apiPath],
              };
            },
            async dispose() {},
          };
        },
      },
    },
    apiBaseUrl: "http://127.0.0.1:7800",
    storageStatePath: "/tmp/storage.json",
    scenarios: [
      {
        fixture_kind: "frontstage_page",
        path_template: "/:slug/pages/:page_id",
      },
      {
        fixture_kind: "workflow_application",
        path_template: "/applications/:application_id/orchestration",
      },
    ],
  });

  assert.equal(scenarios[0].fixture_path, "/task-planning-ft/pages/page-1");
  assert.equal(
    scenarios[1].fixture_path,
    "/applications/workflow-1/orchestration",
  );
});

test("DV-F01 robust regression activates after five same-reference samples", () => {
  const history = [100, 101, 102, 103, 104];
  const accepted = evaluateBudget({ value: 103, absoluteMax: 1999, history });
  const rejected = evaluateBudget({ value: 140, absoluteMax: 1999, history });

  assert.equal(accepted.historyStatus, "active");
  assert.equal(accepted.ok, true);
  assert.equal(rejected.ok, false);
});

test("DV-F01 Tarjan SCC and condensation reachability preserve cycles", () => {
  const graph = new Map([
    ["a", ["b"]],
    ["b", ["a", "c"]],
    ["c", []],
  ]);
  const components = stronglyConnectedComponents(graph);
  assert.deepEqual(components, [["c"], ["a", "b"]]);
  const reachable = reachableComponentSets([new Set(), new Set([0])]);
  assert.deepEqual([...reachable[1]], [1, 0]);
});

test("DV-F01 robust budget combines catastrophe ceiling and Median MAD regression", () => {
  assert.equal(robustLimit([100, 101, 102, 103]), null);
  assert.equal(evaluateBudget({ value: 1999, absoluteMax: 1999 }).ok, true);
  assert.equal(evaluateBudget({ value: 2000, absoluteMax: 1999 }).ok, false);
  assert.equal(
    evaluateBudget({
      value: 140,
      absoluteMax: 1999,
      history: [100, 101, 102, 103, 104],
    }).ok,
    false,
  );
});

test("DV-F01 cache identity changes only with declared runtime inputs", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "dev-experience-cache-"));
  const lock = path.join(root, "pnpm-lock.yaml");
  fs.writeFileSync(lock, "lock-v1");
  const first = cacheIdentity({
    files: [lock],
    env: { VITE_DEV_ALLOWED_HOSTS: "example.test" },
  });
  const same = cacheIdentity({
    files: [lock],
    env: { VITE_DEV_ALLOWED_HOSTS: "example.test" },
  });
  const changedEnv = cacheIdentity({
    files: [lock],
    env: { VITE_DEV_ALLOWED_HOSTS: "other.test" },
  });
  fs.writeFileSync(lock, "lock-v2");
  const changedLock = cacheIdentity({
    files: [lock],
    env: { VITE_DEV_ALLOWED_HOSTS: "example.test" },
  });
  assert.equal(first, same);
  assert.notEqual(first, changedEnv);
  assert.notEqual(first, changedLock);
});

test("DV-F01 resource receipt counts module graph cost and failures", () => {
  const result = summarizeResources(
    [
      {
        name: "http://localhost/src/main.tsx",
        decodedBodySize: 100,
        transferSize: 80,
      },
      {
        name: "http://localhost/assets/logo.svg",
        decodedBodySize: 50,
        transferSize: 40,
      },
    ],
    [{ url: "https://example.test/fail.js", error: "CERT" }],
  );
  assert.equal(result.moduleRequestCount, 1);
  assert.equal(result.decodedBytes, 150);
  assert.equal(result.failedRequests.length, 1);
});

test("DV-F01 profile history is bounded for Median MAD regression", () => {
  const history = updateHistory(
    {
      "route\0cold\0moduleRequestCount": Array.from(
        { length: 20 },
        (_value, index) => index,
      ),
    },
    [
      {
        scenario: "route",
        phase: "cold",
        moduleRequestCount: 20,
        decodedBytes: 100,
        failedRequests: [],
      },
    ],
  );
  assert.deepEqual(
    history["route\0cold\0moduleRequestCount"],
    Array.from({ length: 20 }, (_value, index) => index + 1),
  );
});
