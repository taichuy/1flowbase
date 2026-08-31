const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");

const repoRoot = path.resolve(__dirname, "../../../..");

test("MDP-012 mounts the local production dist over the existing web image", () => {
  const override = fs.readFileSync(
    path.join(
      repoRoot,
      "scripts/node/production-bundle-profile/fixtures/local-dist.compose.yaml",
    ),
    "utf8",
  );
  const harness = fs.readFileSync(
    path.join(
      repoRoot,
      "scripts/node/production-bundle-profile/local-dist-container.js",
    ),
    "utf8",
  );

  assert.match(
    override,
    /dist\/index\.html:\/usr\/share\/nginx\/html\/index\.html:ro/u,
  );
  assert.match(override, /dist\/assets:\/usr\/share\/nginx\/html\/assets:ro/u);
  assert.match(override, /dist\/icons:\/usr\/share\/nginx\/html\/icons:ro/u);
  assert.doesNotMatch(
    override,
    /dist:\/usr\/share\/nginx\/html:ro/u,
    "the parent mount must remain writable so external-npm can mount below it",
  );
  assert.match(harness, /["']--no-build["']/u);
  assert.match(harness, /["']--pull["'],[\s\S]*["']never["']/u);
  assert.match(harness, /["']--no-deps["']/u);
  assert.match(harness, /["']restore["']/u);
  assert.match(harness, /["']docker-compose["']/u);
  assert.match(harness, /["']compose["'], ["']version["']/u);
});
