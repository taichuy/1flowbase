"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const {
  conversationTestInvocations,
  dockerDatabaseContract,
  parseArgs,
  testFiles,
} = require("../cli");

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

test("quality gate limits conversation Cargo probes to deterministic library suites", () => {
  const invocations = conversationTestInvocations("/repo");
  assert.deepEqual(
    invocations.map(({ name, args }) => [name, args.at(-1)]),
    [
      ["control-plane-conversation-tests", "application_public_api"],
      [
        "api-server-protocol-projection-tests",
        "routes::application_public_api::compat_sse::tests::protocol_projection",
      ],
      [
        "api-server-responses-websocket-tests",
        "routes::application_public_api::responses_websocket::tests",
      ],
      [
        "api-server-callback-adapter-tests",
        "routes::application_public_api::callback_adapter::tests",
      ],
      [
        "api-server-native-sse-tests",
        "routes::application_public_api::sse::tests",
      ],
      [
        "api-server-terminal-fallback-tests",
        "routes::application_public_api::stream_terminal_fallback::tests",
      ],
    ],
  );
  for (const { args } of invocations) {
    assert.equal(args.includes("--lib"), true);
    assert.equal(args.includes("--tests"), false);
    assert.equal(args.includes("--all-targets"), false);
  }
  assert.equal(invocations[0].args.at(-1), "application_public_api");
  for (const { args } of invocations.slice(1)) {
    assert.notEqual(args.at(-1), "application_public_api");
  }
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
