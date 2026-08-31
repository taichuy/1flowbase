#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");

const { percentile } = require("./core.js");

function optionValues(name) {
  return process.argv.flatMap((value, index) =>
    value === name && process.argv[index + 1] ? [process.argv[index + 1]] : [],
  );
}

function optionValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

const inputs = optionValues("--input").map((input) => path.resolve(input));
if (inputs.length === 0) {
  throw new Error("At least one --input receipt is required");
}
const receipts = inputs.map((input) =>
  JSON.parse(fs.readFileSync(input, "utf8")),
);
const interactionReadySamples = receipts.map(({ interaction }, index) => {
  if (!interaction || typeof interaction.readyMs !== "number") {
    throw new Error(`Receipt '${inputs[index]}' has no interaction sample`);
  }
  return interaction.readyMs;
});
const summary = {
  schemaVersion: "1flowbase.production-interaction-summary/v1",
  sampleCount: receipts.length,
  allPassed: receipts.every(({ ok }) => ok === true),
  interactionReadyMs: {
    min: Math.min(...interactionReadySamples),
    median: percentile(interactionReadySamples, 0.5),
    p95: percentile(interactionReadySamples, 0.95),
    max: Math.max(...interactionReadySamples),
  },
  receipts: inputs,
};
const output = `${JSON.stringify(summary, null, 2)}\n`;
const outputPath = optionValue("--output");
if (outputPath) {
  fs.mkdirSync(path.dirname(path.resolve(outputPath)), { recursive: true });
  fs.writeFileSync(path.resolve(outputPath), output);
}
process.stdout.write(output);
process.exitCode = summary.allPassed ? 0 : 1;
