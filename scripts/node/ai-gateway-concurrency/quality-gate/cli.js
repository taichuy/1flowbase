#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const { runWorkflowContract } = require("../workflow-contract/runner");

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
    "workflow-contract",
    "wire-audit",
    "mock-upstream",
    "gateway-fixture",
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

  const mainSourceSha = command(
    repoRoot,
    artifactRoot,
    "main-source-sha",
    "git",
    ["rev-parse", "HEAD"],
  );
  const officialSourceSha = command(
    repoRoot,
    artifactRoot,
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
  if (officialSourceSha !== paired.official_plugins.revision) {
    throw new Error(
      `official provider source must match paired revision ${paired.official_plugins.revision}`,
    );
  }
  const rustcVersion = command(
    repoRoot,
    artifactRoot,
    "rustc-version",
    "rustc",
    ["-vV"],
  );
  const hostTarget = /^host: (.+)$/mu.exec(rustcVersion)?.[1];
  if (!hostTarget) throw new Error("rustc host target is unavailable");

  command(repoRoot, artifactRoot, "protocol-structural-tests", "node", [
    "--test",
    ...testFiles(repoRoot),
  ]);
  command(repoRoot, artifactRoot, "control-plane-conversation-tests", "cargo", [
    "test",
    "--manifest-path",
    path.join(repoRoot, "api/Cargo.toml"),
    "-p",
    "control-plane",
    "application_public_api",
  ]);
  command(repoRoot, artifactRoot, "api-server-conversation-tests", "cargo", [
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
    command(repoRoot, artifactRoot, `${providerCode}-provider-build`, "cargo", [
      "build",
      "--manifest-path",
      path.join(pluginRoot, "Cargo.toml"),
      "--release",
      "--locked",
      "--target",
      hostTarget,
    ]);
    command(
      repoRoot,
      artifactRoot,
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
  command(repoRoot, artifactRoot, "gateway-build", "cargo", [
    "build",
    "--manifest-path",
    path.join(repoRoot, "api/Cargo.toml"),
    "--release",
    "-p",
    "api-server",
    "-p",
    "plugin-runner",
  ]);

  const result = await runWorkflowContract({
    mainSourceSha,
    officialSourceSha,
    profile: "characterize",
    repoRoot,
    databaseUrl: rawOptions.databaseUrl,
    apiServerBin: path.join(repoRoot, "api/target/release/api-server"),
    pluginRunnerBin: path.join(repoRoot, "api/target/release/plugin-runner"),
    openaiPackageDir: path.join(packageRoot, "openai"),
    anthropicPackageDir: path.join(packageRoot, "anthropic"),
    hostTarget,
  });
  fs.writeFileSync(
    path.join(artifactRoot, "quality-gate.json"),
    `${JSON.stringify(
      {
        schema_version: "1flowbase.ai-gateway-quality-gate/v1",
        status: result.status,
        main_source_sha: mainSourceSha,
        official_source_sha: officialSourceSha,
        host_target: hostTarget,
        protocol_result: result,
        client_diagnostics: "non-blocking-local-only",
      },
      null,
      2,
    )}\n`,
  );
  return result;
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

module.exports = { main, parseArgs, runQualityGate, testFiles };
