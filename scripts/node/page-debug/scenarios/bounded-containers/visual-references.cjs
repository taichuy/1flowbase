"use strict";
const fs = require("node:fs/promises");
const path = require("node:path");
const crypto = require("node:crypto");
const assert = require("node:assert/strict");

const roles = ["canvas-overview", "loop-settings", "iteration-settings"];
async function prepareReferences(manifestPath, outputDir) {
  assert.ok(
    manifestPath,
    "--reference-manifest is required for the original-user-image comparison evidence",
  );
  const manifest = JSON.parse(await fs.readFile(manifestPath, "utf8"));
  assert.equal(
    manifest.references?.length,
    3,
    "three original user screenshots required",
  );
  await fs.mkdir(path.join(outputDir, "references"), { recursive: true });
  const records = [];
  for (const role of roles) {
    const entry = manifest.references.find(
      (reference) => reference.role === role,
    );
    assert.ok(
      entry && path.isAbsolute(entry.path),
      `absolute private reference path required for ${role}`,
    );
    const bytes = await fs.readFile(entry.path);
    assert.equal(
      bytes.subarray(0, 8).toString("hex"),
      "89504e470d0a1a0a",
      `${role} must be an original PNG`,
    );
    const file = `references/${role}.png`;
    await fs.writeFile(path.join(outputDir, file), bytes, { mode: 0o600 });
    records.push({
      role,
      file,
      sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
      width: bytes.readUInt32BE(16),
      height: bytes.readUInt32BE(20),
      source: "user-provided original screenshot",
      comparison: "pending independent visual review",
    });
  }
  await fs.writeFile(
    path.join(outputDir, "visual-comparison.md"),
    `# Original reference comparison\n\nOriginal screenshots are retained byte-for-byte with SHA-256 in receipt.json. Automatic capture is not a visual PASS. QA must view each original and the candidate screenshots and record differences.\n\n| Original | Candidate | Compare |\n|---|---|---|\n| [Canvas](references/canvas-overview.png) | [Three-level overview](02-desktop-overview.png) | White rounded container, cyan type icon, pale dotted body, internal start/edges, same-screen nesting |\n| [Loop](references/loop-settings.png) | [Loop inspector](02a-loop-settings.png) | Blue selection outline; loop variable/condition/max-round hierarchy and nonmodal inspector |\n| [Iteration](references/iteration-settings.png) | [Iteration inspector](02b-iteration-settings.png) | Input/output, parallelism, error behavior and flattening controls |\n\nDeadline controls are approved additions. Different fixture node types, app chrome, text and graph topology are expected. Do not pixel-match an LLM sample to a template fixture or treat screenshot presence as proof of execution.\n`,
    { mode: 0o600 },
  );
  return records;
}
module.exports = { prepareReferences };
