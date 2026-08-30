const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

const {
  cacheIdentity,
  evaluateBudget,
  parseOptimizeDepsInclude,
  reachableComponentSets,
  robustLimit,
  stronglyConnectedComponents,
  summarizeResources,
  updateHistory,
} = require("../core.js");

test("DV-F01 optimize dependency fixture rejects names outside optimizeDeps include", () => {
  const source = `
    const label = 'antd-vendor';
    export default { optimizeDeps: { include: ['react'] } };
  `;
  assert.deepEqual(parseOptimizeDepsInclude(source), ["react"]);
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
