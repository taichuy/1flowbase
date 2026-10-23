const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");
const frontstageApiPath = path.join(
  repoRoot,
  "web",
  "app",
  "src",
  "features",
  "frontstage",
  "api",
  "block-catalog.ts",
);
const apiClientPath = path.join(
  repoRoot,
  "web",
  "packages",
  "api-client",
  "src",
  "index.ts",
);
const transportPath = path.join(
  repoRoot,
  "web",
  "packages",
  "api-client",
  "src",
  "transport.ts",
);
const envExamplePath = path.join(repoRoot, "web", "app", ".env.example");
const viteConfigPath = path.join(repoRoot, "web", "app", "vite.config.ts");

test("frontend API defaults use same-origin requests with development proxy fallback", () => {
  const frontstageApiSource = fs.readFileSync(frontstageApiPath, "utf8");
  const apiClientSource = fs.readFileSync(apiClientPath, "utf8");
  const transportSource = fs.readFileSync(transportPath, "utf8");
  const envExampleSource = fs.readFileSync(envExamplePath, "utf8");
  const viteConfigSource = fs.readFileSync(viteConfigPath, "utf8");

  assert.match(frontstageApiSource, /getDefaultApiBaseUrl/u);
  assert.match(apiClientSource, /transport/u);
  assert.match(transportSource, /locationLike\?\.hostname/u);
  assert.match(transportSource, /locationLike\.origin/u);
  assert.doesNotMatch(transportSource, /:7800/u);
  assert.match(viteConfigSource, /VITE_API_PROXY_TARGET/u);
  assert.match(viteConfigSource, /http:\/\/127\.0\.0\.1:7800/u);
  assert.match(envExampleSource, /current frontend origin/u);
});
