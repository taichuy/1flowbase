#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");

const COMPONENTS = {
  web: {
    manifest: "web/app/package.json",
    readVersion(file) {
      return readWebPackageVersion(fs.readFileSync(file, "utf8"));
    },
    readVersionSource: readWebPackageVersion,
  },
  "api-server": {
    manifest: "api/apps/api-server/Cargo.toml",
    readVersion: readCargoPackageVersion,
    readVersionSource: readCargoPackageVersionSource,
  },
};

function readWebPackageVersion(source) {
  const version = JSON.parse(source).version;
  if (!version) {
    throw new Error("package.json must declare a version");
  }
  return version;
}

function readCargoPackageVersion(file) {
  return readCargoPackageVersionSource(fs.readFileSync(file, "utf8"), file);
}

function readCargoPackageVersionSource(source, label = "Cargo.toml") {
  const match = source.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error(`${label} must declare an explicit package version`);
  }
  return match[1];
}

function fail(message) {
  console.error(message);
  process.exit(1);
}

const [, , commandOrComponent, maybeComponentOrTag] = process.argv;

if (commandOrComponent === "print-tag") {
  const component = maybeComponentOrTag;
  const config = COMPONENTS[component];
  if (!config) {
    fail(`Unknown component: ${component}`);
  }
  const repoRoot = path.resolve(__dirname, "..", "..", "..");
  const manifestPath = path.join(repoRoot, config.manifest);
  console.log(`v${config.readVersion(manifestPath)}`);
  process.exit(0);
}

if (commandOrComponent === "read-stdin-version") {
  const component = maybeComponentOrTag;
  const config = COMPONENTS[component];
  if (!config) {
    fail(`Unknown component: ${component}`);
  }

  let source = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk) => {
    source += chunk;
  });
  process.stdin.on("end", () => {
    try {
      console.log(config.readVersionSource(source));
    } catch (error) {
      fail(error.message);
    }
  });
  return;
}

const component = commandOrComponent;
const imageTag = maybeComponentOrTag;

if (!component || !imageTag) {
  fail(
    "Usage: node scripts/node/container-image-security/verify-version.js <web|api-server> <vX.Y.Z>",
  );
}

const config = COMPONENTS[component];
if (!config) {
  fail(`Unknown component: ${component}`);
}

if (!/^v\d+\.\d+\.\d+([-.][0-9A-Za-z.-]+)?$/.test(imageTag)) {
  fail(
    `Invalid image tag: ${imageTag}. Expected vX.Y.Z, optionally with a Docker-safe suffix.`,
  );
}

const repoRoot = path.resolve(__dirname, "..", "..", "..");
const manifestPath = path.join(repoRoot, config.manifest);
const manifestVersion = config.readVersion(manifestPath);
const expectedTag = `v${manifestVersion}`;

if (imageTag !== expectedTag) {
  fail(
    `${component} image tag ${imageTag} does not match ${config.manifest} version ${manifestVersion}. Expected ${expectedTag}.`,
  );
}

console.log(`${component} ${imageTag} matches ${config.manifest}`);
