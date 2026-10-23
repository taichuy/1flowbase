"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const {
  assertExecutedCargoTests,
  artifactBytes,
  boundedCommandLog,
  canRunWorkflowContract,
  dockerDatabaseContract,
  gatewayBuildInvocation,
  officialProviderTestInvocations,
  parseArgs,
} = require("../cli");

test("Gateway runtime build shares the Rust test profile and binary path", () => {
  assert.deepEqual(gatewayBuildInvocation("/repo"), {
    args: [
      "build", "--manifest-path", "/repo/api/Cargo.toml",
      "--profile", "test", "-p", "api-server", "--bin", "api-server",
    ],
    binary: "/repo/api/target/debug/api-server",
  });
});

test("quality gate measures the exact bounded artifact inventory", () => {
  const root = fs.mkdtempSync(path.join(require("node:os").tmpdir(), "gate-artifact-"));
  fs.mkdirSync(path.join(root, "nested"));
  fs.writeFileSync(path.join(root, "a.log"), "1234");
  fs.writeFileSync(path.join(root, "nested/b.json"), "123456");
  assert.equal(artifactBytes([root]), 10);
});

test("quality gate bounds each command log without hiding both failure edges", () => {
  const value = `head-marker${"x".repeat(3 * 1024 * 1024)}tail-marker`;
  const bounded = boundedCommandLog(value);
  assert.equal(Buffer.byteLength(bounded) <= 2 * 1024 * 1024, true);
  assert.match(bounded, /^head-marker/u);
  assert.match(bounded, /tail-marker$/u);
  assert.match(bounded, /log truncated/u);
});

test("quality gate redacts credential-bearing URIs from command artifacts", () => {
  const bounded = boundedCommandLog(
    "database=postgres://fixture:secret@127.0.0.1:5432/gate endpoint=http://public.invalid/path",
  );
  assert.equal(bounded.includes("fixture:secret"), false);
  assert.match(bounded, /postgres:\/\/<redacted>@127\.0\.0\.1:5432\/gate/u);
  assert.match(bounded, /http:\/\/public\.invalid\/path/u);
});

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

test("quality gate runs all six blocking official provider suites from the paired source", () => {
  assert.deepEqual(
    officialProviderTestInvocations("/official").map(({ name, args }) => [
      name,
      args,
    ]),
    ["openai", "anthropic", "aliyun_bailian", "deepseek", "gemini", "openai_compatible"].map((provider) => [
      `${provider}-provider-tests`,
      [
        "test",
        "--manifest-path",
        `/official/runtime-extensions/@taichuy/${provider}/Cargo.toml`,
        "--locked",
      ],
    ]),
  );
});

test("Gateway mock workflow proceeds when its built inputs are ready despite an independent provider test failure", () => {
  const ready = {
    database: { url: "postgres://fixture@localhost/gate" },
    gatewayBuilt: true,
    packagedProviders: new Set(["openai", "anthropic", "openai_compatible"]),
    mainSourceSha: "gateway-candidate",
    officialSourceSha: "paired-provider-revision",
    pairedRevision: "paired-provider-revision",
    hostTarget: "x86_64-unknown-linux-gnu",
  };
  assert.equal(canRunWorkflowContract(ready), true);
  assert.equal(canRunWorkflowContract({ ...ready, packagedProviders: new Set(["openai", "anthropic"]) }), false);
  assert.equal(canRunWorkflowContract({ ...ready, gatewayBuilt: false }), false);
  assert.equal(canRunWorkflowContract({ ...ready, officialSourceSha: null }), false);
  assert.equal(canRunWorkflowContract({ ...ready, officialSourceSha: "different-revision" }), false);
});

test("quality gate pins the four blocking transports without implicit enum expansion", () => {
  const source = fs.readFileSync(
    path.resolve(__dirname, "../../workflow-contract/runner.js"),
    "utf8",
  );
  for (const label of [
    "OpenAI Chat",
    "Anthropic",
    "Responses SSE",
    "Responses WebSocket",
  ]) {
    assert.match(source, new RegExp(`label: '${label}'`, "u"));
  }
  const inventorySource = source.slice(
    source.indexOf("const BLOCKING_TRANSPORTS"),
    source.indexOf("function protocolOracleInventory"),
  );
  assert.doesNotMatch(inventorySource, /Object\.values/u);
});

function assertVendoredSwagger(cargo) {
  // The workspace uses an inline TOML dependency; only its feature contract is
  // relevant here. A version upgrade must not change this offline build check.
  const dependency = cargo.match(/^utoipa-swagger-ui\s*=\s*\{([^}]+)\}/mu);
  assert.ok(dependency, "workspace must declare utoipa-swagger-ui");
  const features = dependency[1].match(/\bfeatures\s*=\s*\[([^\]]*)\]/u);
  assert.ok(features, "Swagger UI must declare its build features");
  const names = [...features[1].matchAll(/["']([^"']+)["']/gu)].map((match) => match[1]);
  assert.ok(names.includes("vendored"), "Swagger UI must enable vendored assets");
  assert.ok(names.includes("axum"), "Swagger UI must enable Axum integration");
}

test("release gate vendors Swagger UI instead of downloading a GitHub archive during Cargo build", () => {
  const repoRoot = path.resolve(__dirname, "../../../../../");
  assertVendoredSwagger(fs.readFileSync(path.join(repoRoot, "api/Cargo.toml"), "utf8"));
  for (const version of ["8", "10.0.1", "99.1.0"]) {
    assertVendoredSwagger(`utoipa-swagger-ui = { version = "${version}", features = ["vendored", "axum"] }`);
  }
  assert.throws(() => assertVendoredSwagger(
    'utoipa-swagger-ui = { version = "10.0.1", features = ["axum"] }',
  ), /must enable vendored assets/u);
  assert.throws(() => assertVendoredSwagger(
    'other = { features = ["axum", "vendored"] }',
  ), /must declare utoipa-swagger-ui/u);
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


test("AC009 refuses successful Cargo commands with no actual passing tests", () => {
  assert.throws(() => assertExecutedCargoTests("test result: ok. 0 passed; 0 failed;", "empty"), /no executed/);
  assert.throws(() => assertExecutedCargoTests("Finished test profile", "missing"), /no executed/);
  assert.doesNotThrow(() => assertExecutedCargoTests("test result: ok. 2 passed; 0 failed;\ntest result: ok. 0 passed; 0 failed;", "actual"));
});
