#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const repoRoot = path.resolve(__dirname, "../../..");
const composeDirectory = path.join(repoRoot, "deploy/docker");
const baseCompose = path.join(composeDirectory, "docker-compose.yaml");
const mountCompose = path.join(__dirname, "fixtures/local-dist.compose.yaml");
const distIndex = path.join(repoRoot, "web/app/dist/index.html");
const action = process.argv[2] ?? "up";

function composeCommand() {
  const plugin = spawnSync("docker", ["compose", "version"], {
    encoding: "utf8",
  });
  return plugin.status === 0
    ? { executable: "docker", prefix: ["compose"] }
    : { executable: "docker-compose", prefix: [] };
}

if (!["config", "restore", "up"].includes(action)) {
  throw new Error("Expected action: config, up, or restore");
}
if (action === "up" && !fs.existsSync(distIndex)) {
  throw new Error(
    "web/app/dist/index.html is missing; build the production frontend first",
  );
}

const files =
  action === "restore"
    ? ["-f", baseCompose]
    : ["-f", baseCompose, "-f", mountCompose];
const command =
  action === "config"
    ? ["compose", ...files, "config"]
    : [
        "compose",
        ...files,
        "up",
        "-d",
        "--no-deps",
        "--no-build",
        "--pull",
        "never",
        "--force-recreate",
        "web",
      ];
const compose = composeCommand();
const result = spawnSync(
  compose.executable,
  [...compose.prefix, ...command.slice(1)],
  {
    cwd: composeDirectory,
    encoding: "utf8",
    stdio: action === "config" ? "pipe" : "inherit",
  },
);
if (action === "config" && result.status === 0) {
  process.stdout.write(result.stdout);
}
if (action === "config" && result.status !== 0 && result.stderr) {
  process.stderr.write(result.stderr);
}
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
