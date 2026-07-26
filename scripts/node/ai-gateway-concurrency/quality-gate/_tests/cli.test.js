"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const { dockerDatabaseContract, parseArgs, testFiles } = require("../cli");

test("quality gate exposes one explicit command with local source and database inputs", () => {
  assert.deepEqual(
    parseArgs([
      "run",
      "--repo-root",
      "/repo",
      "--official-source-root",
      "/official",
      "--database-url",
      "postgres://localhost/gate",
    ]),
    {
      repoRoot: "/repo",
      officialSourceRoot: "/official",
      databaseUrl: "postgres://localhost/gate",
    },
  );
  assert.throws(() => parseArgs(["run"]), /required/u);
});

test("quality gate inventory contains the four blocking protocol harness suites", () => {
  const repoRoot = path.resolve(__dirname, "../../../../../");
  const files = testFiles(repoRoot);
  for (const suite of [
    "protocol-oracle",
    "responses-websocket-acceptance",
    "characterize",
    "workflow-contract",
    "wire-audit",
    "mock-upstream",
    "gateway-fixture",
    "quality-gate",
  ]) {
    assert.ok(
      files.some((file) => file.includes(`/${suite}/_tests/`)),
      suite,
    );
  }
  assert.equal(files.some((file) => file.includes("/local-client-acceptance/")), false);
});

test("quality gate pins the four blocking transports without implicit enum expansion", () => {
  const source = fs.readFileSync(path.resolve(__dirname, "../../workflow-contract/runner.js"), "utf8");
  for (const label of ["OpenAI Chat", "Anthropic", "Responses SSE", "Responses WebSocket"]) {
    assert.match(source, new RegExp(`label: '${label}'`, "u"));
  }
  const inventorySource = source.slice(source.indexOf("const BLOCKING_TRANSPORTS"), source.indexOf("function protocolOracleInventory"));
  assert.doesNotMatch(inventorySource, /Object\.values/u);
});

test("release gate vendors Swagger UI instead of downloading a GitHub archive during Cargo build", () => {
  const repoRoot = path.resolve(__dirname, "../../../../../");
  const cargo = fs.readFileSync(path.join(repoRoot, "api/Cargo.toml"), "utf8");
  assert.match(cargo, /utoipa-swagger-ui = \{ version = "8", features = \["axum", "vendored"\] \}/u);
});

test("quality gate derives one owned temporary database from the loopback Docker service", () => {
  assert.deepEqual(
    dockerDatabaseContract(
      "postgres://postgres:secret@127.0.0.1:35432/1flowbase",
      "container-1\n",
    ),
    { container: "container-1", host: "127.0.0.1", port: 35432 },
  );
  assert.throws(
    () =>
      dockerDatabaseContract(
        "postgres://postgres:secret@db.internal:5432/1flowbase",
        "container-1",
      ),
    /loopback/u,
  );
  assert.throws(
    () =>
      dockerDatabaseContract(
        "postgres://postgres:secret@127.0.0.1:5432/1flowbase",
        "one\ntwo",
      ),
    /exactly one/u,
  );
});
