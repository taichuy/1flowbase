"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const {
  artifactBytes,
  boundedCommandLog,
  conversationTestInvocations,
  dockerDatabaseContract,
  officialProviderTestInvocations,
  parseArgs,
  testFiles,
} = require("../cli");

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

test("quality gate inventory contains protocol and local-client contract suites", () => {
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
  assert.equal(
    files.some((file) => file.includes("/local-client-acceptance/")),
    true,
  );
});

test("quality gate limits conversation Cargo probes to one owned database and deterministic library suites", () => {
  const databaseUrl = "postgres://gate@127.0.0.1:35432/owned";
  const invocations = conversationTestInvocations("/repo", databaseUrl);
  assert.deepEqual(
    invocations.map(({ name, args }) => [name, args.at(-1)]),
    [
      [
        "plugin-framework-count-tokens-estimator-total-corpus",
        "d1_p03_generic_estimator_is_total_for_canonical_prompt_block_families",
      ],
      ["plugin-framework-count-tokens-contract-tests", "count_tokens"],
      ["plugin-runner-count-tokens-totality-tests", "count_tokens"],
      ["orchestration-runtime-count-tokens-terminal-tests", "count_tokens"],
      ["control-plane-count-tokens-route-tests", "count_tokens"],
      ["api-server-count-tokens-envelope-tests", "count_tokens"],
      ["plugin-framework-message-block-contract-tests", "root_1534"],
      ["orchestration-runtime-semantic-route-tests", "root_1534"],
      ["orchestration-runtime-upstream-error-tests", "upstream"],
      ["control-plane-conversation-tests", "application_public_api"],
      [
        "control-plane-live-provider-error-tests",
        "provider_error_after_live_delta_drains_runtime_event_stream_forwarding",
      ],
      [
        "orchestration-runtime-anthropic-callback-retry-tests",
        "anthropic_callback_retry",
      ],
      [
        "orchestration-runtime-cross-provider-image-llm-tests",
        "ac_001_cross_provider_image_llm_does_not_inherit_parent_reasoning_semantics",
      ],
      ["control-plane-answer-node-truth-tests", "ac_004_answer_node_truth"],
      ["api-server-answer-node-truth-tests", "ac_004_answer_node_truth"],
      [
        "api-server-translation-protocol-persistence-tests",
        "compatibility_mode",
      ],
      [
        "storage-postgres-protocol-context-migration-tests",
        "protocol_context_migration_tests",
      ],
      [
        "api-server-protocol-projection-tests",
        "routes::application_public_api::compat_sse::tests::protocol_projection",
      ],
      [
        "api-server-public-stream-causality-tests",
        "_tests::application_public_api::compat_routes::streaming::ac_live_causality",
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
  for (const { options } of invocations) {
    assert.equal(options.env.API_DATABASE_URL, databaseUrl);
    assert.equal(options.env.DATABASE_URL, databaseUrl);
    assert.equal(options.env.BOOTSTRAP_ROOT_ACCOUNT, "root");
    assert.equal(options.env.BOOTSTRAP_ROOT_PASSWORD, "change-me");
  }
  const applicationPublicApi = invocations.find(
    ({ name }) => name === "control-plane-conversation-tests",
  );
  assert.equal(applicationPublicApi.args.at(-1), "application_public_api");
  for (const { args } of invocations.filter(
    ({ name }) => name !== "control-plane-conversation-tests",
  )) {
    assert.notEqual(args.at(-1), "application_public_api");
  }
});

test("F03 Cargo filters execute the estimator corpus, fault fixture, and Native typed receipt tests", () => {
  const repoRoot = path.resolve(__dirname, "../../../../../");
  const invocations = conversationTestInvocations(
    repoRoot,
    "postgres://gate@127.0.0.1:35432/owned",
  );
  const requiredTests = [
    {
      packageName: "plugin-framework",
      testName: "d1_p03_generic_estimator_is_total_for_canonical_prompt_block_families",
      source: "api/crates/plugin-framework/src/_tests/provider_contract_tests.rs",
    },
    {
      packageName: "plugin-framework",
      testName: "d1_p03_count_tokens_estimator_fault_injection_projects_typed_fallback_zero",
      source: "api/crates/plugin-framework/src/provider_count_tokens_estimator.rs",
    },
    {
      packageName: "api-server",
      testName: "d3_p12_count_tokens_native_blocking_response_exposes_typed_operation_terminal",
      source: "api/apps/api-server/src/_tests/application_public_api/native_routes.rs",
    },
  ];

  for (const required of requiredTests) {
    const testSource = fs.readFileSync(path.join(repoRoot, required.source), "utf8");
    assert.ok(
      testSource.includes(`fn ${required.testName}(`),
      `${required.testName} must exist in the real Rust test source`,
    );
    const matching = invocations.filter(({ args }) => {
      const packageIndex = args.indexOf("-p");
      const filter = args.at(-1);
      return args[packageIndex + 1] === required.packageName
        && required.testName.includes(filter);
    });
    assert.equal(
      matching.length,
      1,
      `${required.testName} must be selected by exactly one Cargo command`,
    );
  }
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

test("release gate vendors Swagger UI instead of downloading a GitHub archive during Cargo build", () => {
  const repoRoot = path.resolve(__dirname, "../../../../../");
  const cargo = fs.readFileSync(path.join(repoRoot, "api/Cargo.toml"), "utf8");
  assert.match(
    cargo,
    /utoipa-swagger-ui = \{ version = "8", features = \["axum", "vendored"\] \}/u,
  );
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
