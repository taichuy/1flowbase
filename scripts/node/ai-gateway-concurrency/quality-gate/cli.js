#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const {
  BLOCKING_TRANSPORTS,
  runWorkflowContract,
} = require("../workflow-contract/runner");
const { createDatabase } = require("../local-acceptance/system");

const OFFICIAL_PROVIDER_CODES = Object.freeze([
  "openai",
  "anthropic",
  "openai_compatible",
]);
const MAX_COMMAND_LOG_BYTES = 2 * 1024 * 1024;
const MAX_GATE_ARTIFACT_BYTES = 64 * 1024 * 1024;

function boundedCommandLog(output) {
  const redacted = redactCredentialUris(output);
  const encoded = Buffer.from(redacted);
  if (encoded.length <= MAX_COMMAND_LOG_BYTES) return redacted;
  const marker = Buffer.from(
    `\n[ai-gateway-quality-gate] log truncated from ${encoded.length} bytes\n`,
  );
  const remaining = MAX_COMMAND_LOG_BYTES - marker.length;
  const headLength = Math.floor(remaining / 2);
  return Buffer.concat([
    encoded.subarray(0, headLength),
    marker,
    encoded.subarray(encoded.length - (remaining - headLength)),
  ]).toString("utf8");
}

function redactCredentialUris(output) {
  let cursor = 0;
  let searchFrom = 0;
  let redacted = "";

  while (true) {
    const separator = output.indexOf("://", searchFrom);
    if (separator === -1) break;

    let schemeStart = separator;
    while (schemeStart > cursor && /[a-z0-9+.-]/iu.test(output[schemeStart - 1])) {
      schemeStart -= 1;
    }
    if (
      schemeStart === separator ||
      !/[a-z]/iu.test(output[schemeStart]) ||
      (schemeStart > 0 && /[a-z0-9+.-]/iu.test(output[schemeStart - 1]))
    ) {
      searchFrom = separator + 3;
      continue;
    }

    const authorityStart = separator + 3;
    let authorityEnd = authorityStart;
    while (
      authorityEnd < output.length &&
      !/[\s/?#]/u.test(output[authorityEnd])
    ) {
      authorityEnd += 1;
    }
    const at = output.indexOf("@", authorityStart);
    const colon = output.indexOf(":", authorityStart);
    if (
      at !== -1 &&
      at < authorityEnd &&
      colon > authorityStart &&
      colon < at &&
      colon + 1 < at
    ) {
      redacted += output.slice(cursor, authorityStart);
      redacted += "<redacted>@";
      cursor = at + 1;
    }
    searchFrom = authorityEnd > separator ? authorityEnd : separator + 3;
  }

  return redacted + output.slice(cursor);
}

function artifactBytes(roots) {
  let total = 0;
  const visit = (target) => {
    if (!fs.existsSync(target)) return;
    const stat = fs.statSync(target);
    if (stat.isFile()) {
      total += stat.size;
      return;
    }
    for (const entry of fs.readdirSync(target)) visit(path.join(target, entry));
  };
  for (const root of roots) visit(root);
  return total;
}

function dockerDatabaseContract(databaseUrl, containerIds) {
  const url = new URL(databaseUrl);
  const host = url.hostname;
  const port = Number(url.port || 5432);
  if (!["127.0.0.1", "localhost", "::1"].includes(host)) {
    throw new Error(
      "quality gate PostgreSQL must be a loopback Docker service",
    );
  }
  const containers = containerIds
    .split(/\r?\n/u)
    .map((value) => value.trim())
    .filter(Boolean);
  if (containers.length !== 1) {
    throw new Error(
      `quality gate requires exactly one PostgreSQL container on host port ${port}`,
    );
  }
  return { container: containers[0], host, port };
}

function parseArgs(argv) {
  if (argv[0] !== "run")
    throw new Error(
      "usage: quality-gate/cli.js run --official-source-root <path> --database-url <url>",
    );
  const values = {};
  const fields = new Map([
    ["--repo-root", "repoRoot"],
    ["--official-source-root", "officialSourceRoot"],
    ["--database-url", "databaseUrl"],
  ]);
  for (let index = 1; index < argv.length; index += 2) {
    const field = fields.get(argv[index]);
    const value = argv[index + 1];
    if (!field || !value || value.startsWith("--"))
      throw new Error(`invalid argument: ${argv[index]}`);
    values[field] = value;
  }
  if (!values.officialSourceRoot || !values.databaseUrl)
    throw new Error("official source root and database URL are required");
  return values;
}

function command(root, artifactRoot, name, command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || root,
    env: { ...process.env, ...options.env },
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  const output = `${result.stdout || ""}${result.stderr || ""}`;
  fs.writeFileSync(
    path.join(artifactRoot, `${name}.log`),
    boundedCommandLog(output),
  );
  if (output) process.stdout.write(output);
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(`${name} failed with exit code ${result.status}`);
  return (result.stdout || "").trim();
}

function testFiles(repoRoot) {
  const suites = [
    "protocol-oracle",
    "responses-websocket-acceptance",
    "characterize",
    "workflow-contract",
    "wire-audit",
    "mock-upstream",
    "gateway-fixture",
    "local-acceptance",
    "local-client-acceptance",
    "quality-gate",
  ];
  return suites.flatMap((suite) => {
    const directory = path.join(
      repoRoot,
      "scripts/node/ai-gateway-concurrency",
      suite,
      "_tests",
    );
    return fs
      .readdirSync(directory)
      .filter((file) => file.endsWith(".test.js"))
      .sort()
      .map((file) => path.join(directory, file));
  });
}

function conversationTestInvocations(repoRoot, databaseUrl) {
  const manifestPath = path.join(repoRoot, "api/Cargo.toml");
  const invocation = (name, packageName, filter) => ({
    name,
    options: {
      env: {
        API_DATABASE_URL: databaseUrl,
        BOOTSTRAP_ROOT_ACCOUNT: "root",
        BOOTSTRAP_ROOT_PASSWORD: "change-me",
      },
    },
    args: [
      "test",
      "--manifest-path",
      manifestPath,
      "-p",
      packageName,
      "--lib",
      filter,
    ],
  });
  return [
    invocation(
      "orchestration-runtime-upstream-error-tests",
      "orchestration-runtime",
      "upstream",
    ),
    invocation(
      "control-plane-conversation-tests",
      "control-plane",
      "application_public_api",
    ),
    invocation(
      "control-plane-live-provider-error-tests",
      "control-plane",
      "provider_error_after_live_delta_drains_runtime_event_stream_forwarding",
    ),
    invocation(
      "control-plane-answer-node-truth-tests",
      "control-plane",
      "ac_004_answer_node_truth",
    ),
    invocation(
      "api-server-answer-node-truth-tests",
      "api-server",
      "ac_004_answer_node_truth",
    ),
    invocation(
      "api-server-protocol-projection-tests",
      "api-server",
      "routes::application_public_api::compat_sse::tests::protocol_projection",
    ),
    invocation(
      "api-server-public-stream-causality-tests",
      "api-server",
      "_tests::application_public_api::compat_routes::streaming::ac_live_causality",
    ),
    invocation(
      "api-server-responses-websocket-tests",
      "api-server",
      "routes::application_public_api::responses_websocket::tests",
    ),
    invocation(
      "api-server-callback-adapter-tests",
      "api-server",
      "routes::application_public_api::callback_adapter::tests",
    ),
    invocation(
      "api-server-native-sse-tests",
      "api-server",
      "routes::application_public_api::sse::tests",
    ),
    invocation(
      "api-server-terminal-fallback-tests",
      "api-server",
      "routes::application_public_api::stream_terminal_fallback::tests",
    ),
  ];
}

function officialProviderTestInvocations(officialSourceRoot) {
  return OFFICIAL_PROVIDER_CODES.map((providerCode) => ({
    name: `${providerCode}-provider-tests`,
    args: [
      "test",
      "--manifest-path",
      path.join(
        officialSourceRoot,
        "runtime-extensions/model-providers",
        providerCode,
        "Cargo.toml",
      ),
      "--lib",
      "--locked",
    ],
  }));
}

async function runQualityGate(rawOptions) {
  const repoRoot = path.resolve(rawOptions.repoRoot || process.cwd());
  const officialSourceRoot = path.resolve(rawOptions.officialSourceRoot);
  const artifactRoot = path.join(
    repoRoot,
    "tmp/test-governance/ai-gateway-quality-gate",
  );
  const workflowArtifactRoot = path.join(
    repoRoot,
    "tmp/test-governance/ai-gateway-concurrency",
  );
  const packageRoot = path.join(artifactRoot, "packages");
  fs.rmSync(artifactRoot, { recursive: true, force: true });
  fs.rmSync(workflowArtifactRoot, { recursive: true, force: true });
  fs.mkdirSync(packageRoot, { recursive: true });

  const failures = [];
  const attempt = (name, executable, args, options) => {
    try {
      return command(repoRoot, artifactRoot, name, executable, args, options);
    } catch (error) {
      failures.push({ name, message: error.message });
      return null;
    }
  };
  const mainSourceSha = attempt("main-source-sha", "git", [
    "rev-parse",
    "HEAD",
  ]);
  const officialSourceSha = attempt("official-source-sha", "git", [
    "-C",
    officialSourceRoot,
    "rev-parse",
    "HEAD",
  ]);
  const paired = require(
    path.join(
      repoRoot,
      "scripts/node/ai-gateway-concurrency/workflow-contract/paired-source.lock.json",
    ),
  );
  if (
    officialSourceSha &&
    officialSourceSha !== paired.official_plugins.revision
  ) {
    failures.push({
      name: "paired-source",
      message: `official provider source must match paired revision ${paired.official_plugins.revision}`,
    });
  }
  const rustcVersion = attempt("rustc-version", "rustc", ["-vV"]);
  const hostTarget = /^host: (.+)$/mu.exec(rustcVersion)?.[1];
  if (!hostTarget)
    failures.push({
      name: "rustc-host",
      message: "rustc host target is unavailable",
    });

  const adminDatabaseUrl = new URL(rawOptions.databaseUrl);
  const publishedPort = Number(adminDatabaseUrl.port || 5432);
  const containerIds = attempt("postgres-container", "docker", [
    "ps",
    "--filter",
    `publish=${publishedPort}`,
    "--format",
    "{{.ID}}",
  ]);
  let database = null;
  if (containerIds !== null) {
    try {
      database = createDatabase(
        dockerDatabaseContract(rawOptions.databaseUrl, containerIds),
      );
    } catch (error) {
      failures.push({ name: "database-setup", message: error.message });
    }
  }

  let result = null;
  try {
    attempt("protocol-structural-tests", "node", [
      "--test",
      ...testFiles(repoRoot),
    ]);
    for (const invocation of database
      ? conversationTestInvocations(repoRoot, database.url)
      : []) {
      attempt(invocation.name, "cargo", invocation.args, invocation.options);
    }

    for (const invocation of officialProviderTestInvocations(
      officialSourceRoot,
    )) {
      attempt(invocation.name, "cargo", invocation.args);
    }

    for (const providerCode of OFFICIAL_PROVIDER_CODES) {
      const pluginRoot = path.join(
        officialSourceRoot,
        "runtime-extensions/model-providers",
        providerCode,
      );
      const built = attempt(`${providerCode}-provider-build`, "cargo", [
        "build",
        "--manifest-path",
        path.join(pluginRoot, "Cargo.toml"),
        "--release",
        "--locked",
        "--target",
        hostTarget || "unavailable",
      ]);
      if (built !== null)
        attempt(`${providerCode}-provider-package`, "node", [
          path.join(repoRoot, "scripts/node/plugin/cli.js"),
          "package",
          pluginRoot,
          "--out",
          path.join(packageRoot, providerCode),
          "--runtime-binary",
          path.join(
            pluginRoot,
            "target",
            hostTarget,
            "release",
            `${providerCode}-provider`,
          ),
          "--target",
          hostTarget,
        ]);
    }
    attempt("gateway-build", "cargo", [
      "build",
      "--manifest-path",
      path.join(repoRoot, "api/Cargo.toml"),
      "--release",
      "-p",
      "api-server",
      "-p",
      "plugin-runner",
    ]);

    if (failures.length === 0 && database) {
      try {
        result = await runWorkflowContract({
          mainSourceSha,
          officialSourceSha,
          profile: "characterize",
          repoRoot,
          databaseUrl: database.url,
          apiServerBin: path.join(repoRoot, "api/target/release/api-server"),
          pluginRunnerBin: path.join(
            repoRoot,
            "api/target/release/plugin-runner",
          ),
          openaiPackageDir: path.join(packageRoot, "openai"),
          anthropicPackageDir: path.join(packageRoot, "anthropic"),
          openaiCompatiblePackageDir: path.join(
            packageRoot,
            "openai_compatible",
          ),
          hostTarget,
        });
        if (result.status !== "pass")
          failures.push({
            name: "workflow-contract",
            message: result.error?.message || "blocking workflow failed",
          });
      } catch (error) {
        failures.push({ name: "workflow-contract", message: error.message });
      }
    }
  } finally {
    try {
      await database?.close();
    } catch (error) {
      failures.push({ name: "database-cleanup", message: error.message });
    }
  }
  const evidenceRoots = [
    artifactRoot,
    workflowArtifactRoot,
  ];
  const evidenceBytes = artifactBytes(evidenceRoots);
  if (evidenceBytes > MAX_GATE_ARTIFACT_BYTES) {
    failures.push({
      name: "artifact-budget",
      message: `AI Gateway evidence used ${evidenceBytes} bytes; budget is ${MAX_GATE_ARTIFACT_BYTES}`,
    });
  }
  const secretValues = [
    rawOptions.databaseUrl,
    new URL(rawOptions.databaseUrl).password,
    database?.url,
    database?.url ? new URL(database.url).password : null,
  ].filter(Boolean);
  const publicFailures = failures.map((failure) => ({
    name: failure.name,
    message: secretValues
      .reduce(
        (message, secret) => message.replaceAll(secret, "<redacted>"),
        failure.message,
      )
      .slice(0, 4_000),
  }));
  const gateResult = {
    status: failures.length === 0 ? "pass" : "fail",
    failures: publicFailures,
    workflow: result,
  };
  fs.writeFileSync(
    path.join(artifactRoot, "quality-gate.json"),
    `${JSON.stringify(
      {
        schema_version: "1flowbase.ai-gateway-quality-gate/v1",
        status: gateResult.status,
        main_source_sha: mainSourceSha,
        official_source_sha: officialSourceSha,
        host_target: hostTarget,
        blocking_transports: BLOCKING_TRANSPORTS,
        official_provider_codes: OFFICIAL_PROVIDER_CODES,
        artifact_bytes: evidenceBytes,
        artifact_budget_bytes: MAX_GATE_ARTIFACT_BYTES,
        failures: publicFailures,
        protocol_result: result,
        client_diagnostics: "non-blocking-local-only",
      },
      null,
      2,
    )}\n`,
  );
  return gateResult;
}

async function main(argv = process.argv.slice(2)) {
  const result = await runQualityGate(parseArgs(argv));
  process.stdout.write(`[ai-gateway-quality-gate] ${result.status}\n`);
  return result.status === "pass" ? 0 : 1;
}

if (require.main === module) {
  main()
    .then((status) => {
      process.exitCode = status;
    })
    .catch((error) => {
      process.stderr.write(
        `[ai-gateway-quality-gate] ${error.stack || error.message}\n`,
      );
      process.exitCode = 1;
    });
}

module.exports = {
  conversationTestInvocations,
  artifactBytes,
  boundedCommandLog,
  dockerDatabaseContract,
  main,
  officialProviderTestInvocations,
  parseArgs,
  runQualityGate,
  testFiles,
};
