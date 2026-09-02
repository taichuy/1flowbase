#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");

const { profileProductionBundle } = require("./core.js");

function optionValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

const repoRoot = path.resolve(__dirname, "../../..");
const distDirectory = path.resolve(
  optionValue("--dist") || path.join(repoRoot, "web/app/dist"),
);
const outputPath = optionValue("--output");
const profile = profileProductionBundle(distDirectory);
const receipt = `${JSON.stringify(profile, null, 2)}\n`;

if (outputPath) {
  fs.mkdirSync(path.dirname(path.resolve(outputPath)), { recursive: true });
  fs.writeFileSync(path.resolve(outputPath), receipt);
}

process.stdout.write(receipt);
process.exitCode = profile.ok ? 0 : 1;
