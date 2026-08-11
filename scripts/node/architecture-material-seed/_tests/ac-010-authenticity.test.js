const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const manifest = require("../manifest.json");
const { loadAndParseArchive, resolveAcceptanceZipPath } = require("../core.js");

// AC-010 fixture definition only. Root QA owns execution for the frozen Issue Tree batch.
test("AC-010 authorized archive deterministically derives the authentic material tree", () => {
  const zipPath = resolveAcceptanceZipPath({
    repoRoot: path.resolve(__dirname, "..", "..", "..", ".."),
    sourceEnv: process.env,
  });
  assert.ok(
    fs.existsSync(zipPath),
    `authorized acceptance archive missing: ${zipPath}`,
  );

  const parsed = loadAndParseArchive(zipPath, manifest);

  assert.equal(parsed.archiveSha256, manifest.source.archive_sha256);
  assert.equal(parsed.concatenatedSha256, manifest.source.concatenated_sha256);
  assert.equal(parsed.nodes.filter((node) => node.kind === "root").length, 1);
  assert.equal(
    parsed.nodes.filter((node) => node.kind === "chapter").length,
    20,
  );
  assert.equal(
    parsed.nodes.filter((node) => node.kind === "section").length,
    119,
  );
  assert.deepEqual(
    parsed.sectionsPerChapter,
    manifest.derived.sections_per_chapter,
  );
  assert.equal(parsed.nodes.length, 140);
  assert.equal(
    parsed.nodes.reduce(
      (bytes, node) => bytes + Buffer.byteLength(node.content),
      0,
    ),
    manifest.source.uncompressed_bytes,
  );
});
