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

function dockerDatabaseContract(databaseUrl, containerIds) {
  const url = new URL(databaseUrl);
  const host = url.hostname;
  const port = Number(url.port || 5432);
  if (!["127.0.0.1", "localhost", "::1"].includes(host)) {
    throw new Error("quality gate PostgreSQL must be a loopback Docker service");
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
  fs.writeFileSync(path.join(artifactRoot, `${name}.log`), output);
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

async function runQualityGate(rawOptions) {
  const repoRoot = path.resolve(rawOptions.repoRoot || process.cwd());
  const officialSourceRoot = path.resolve(rawOptions.officialSourceRoot);
  const artifactRoot = path.join(
    repoRoot,
    "tmp/test-governance/ai-gateway-quality-gate",
  );
  const packageRoot = path.join(artifactRoot, "packages");
  fs.rmSync(artifactRoot, { recursive: true, force: true });
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
  const mainSourceSha = attempt(
    "main-source-sha",
    "git",
    ["rev-parse", "HEAD"],
  );
  const officialSourceSha = attempt(
    "official-source-sha",
    "git",
    ["-C", officialSourceRoot, "rev-parse", "HEAD"],
  );
  const paired = require(
    path.join(
      repoRoot,
      "scripts/node/ai-gateway-concurrency/workflow-contract/paired-source.lock.json",
    ),
  );
  if (officialSourceSha && officialSourceSha !== paired.official_plugins.revision) {
    failures.push({
      name: "paired-source",
      message: `official provider source must match paired revision ${paired.official_plugins.revision}`,
    });
  }
  const rustcVersion = attempt(
    "rustc-version",
    "rustc",
    ["-vV"],
  );
  const hostTarget = /^host: (.+)$/mu.exec(rustcVersion)?.[1];
  if (!hostTarget) failures.push({ name: "rustc-host", message: "rustc host target is unavailable" });

  attempt("protocol-structural-tests", "node", [
    "--test",
    ...testFiles(repoRoot),
  ]);
  attempt("control-plane-conversation-tests", "cargo", [
    "test",
    "--manifest-path",
    path.join(repoRoot, "api/Cargo.toml"),
    "-p",
    "control-plane",
    "application_public_api",
  ]);
  attempt("api-server-conversation-tests", "cargo", [
    "test",
    "--manifest-path",
    path.join(repoRoot, "api/Cargo.toml"),
    "-p",
    "api-server",
    "application_public_api",
  ]);

  for (const providerCode of ["openai", "anthropic"]) {
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
    if (built !== null) attempt(
      `${providerCode}-provider-package`,
      "node",
      [
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
      ],
    );
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

  let result = null;
  if (failures.length === 0) {
    const adminDatabaseUrl = new URL(rawOptions.databaseUrl);
    const publishedPort = Number(adminDatabaseUrl.port || 5432);
    const containerIds = attempt(
      "postgres-container",
      "docker",
      ["ps", "--filter", `publish=${publishedPort}`, "--format", "{{.ID}}"],
    );
    let database = null;
    try {
      if (containerIds !== null) {
        database = createDatabase(dockerDatabaseContract(rawOptions.databaseUrl, containerIds));
        result = await runWorkflowContract({
          mainSourceSha,
          officialSourceSha,
          profile: "characterize",
          repoRoot,
          databaseUrl: database.url,
          apiServerBin: path.join(repoRoot, "api/target/release/api-server"),
          pluginRunnerBin: path.join(repoRoot, "api/target/release/plugin-runner"),
          openaiPackageDir: path.join(packageRoot, "openai"),
          anthropicPackageDir: path.join(packageRoot, "anthropic"),
          hostTarget,
        });
        if (result.status !== "pass") failures.push({ name: "workflow-contract", message: result.error?.message || "blocking workflow failed" });
      }
    } catch (error) {
      failures.push({ name: "workflow-contract", message: error.message });
    } finally {
      try { await database?.close(); }
      catch (error) { failures.push({ name: "database-cleanup", message: error.message }); }
    }
  }
  const secretValues = [rawOptions.databaseUrl, new URL(rawOptions.databaseUrl).password].filter(Boolean);
  const publicFailures = failures.map((failure) => ({
    name: failure.name,
    message: secretValues.reduce((message, secret) => message.replaceAll(secret, "<redacted>"), failure.message).slice(0, 4_000),
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
  dockerDatabaseContract,
  main,
  parseArgs,
  runQualityGate,
  testFiles,
};
