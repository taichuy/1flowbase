const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const { EventEmitter } = require("node:events");

const {
  collectHtmlEntryAssets,
  collectStaticImports,
  observeAssetDemand,
  percentile,
  profileAssetFiles,
  profileInteractionAssets,
  profileProductionBundle,
} = require("../core.js");

test("MDP-001 attributes demand when a request starts, independent of response latency", () => {
  const page = new EventEmitter();
  const demand = observeAssetDemand(page);
  const request = {
    url: () => "https://gateway.example/assets/initial-a.js",
    failure: () => null,
  };

  page.emit("request", request);
  const interactionBaseline = new Set(demand.requestedAssets);
  page.emit("response", {
    url: request.url,
    status: () => 200,
  });

  assert.deepEqual([...demand.requestedAssets], ["assets/initial-a.js"]);
  assert.deepEqual(
    [...demand.requestedAssets].filter(
      (asset) => !interactionBaseline.has(asset),
    ),
    [],
  );
  assert.deepEqual(demand.failedAssets, []);
});

function createBundle(files, html) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "bundle-profile-"));
  fs.mkdirSync(path.join(directory, "assets"));
  fs.writeFileSync(path.join(directory, "index.html"), html);
  for (const [file, source] of Object.entries(files)) {
    fs.writeFileSync(path.join(directory, "assets", file), source);
  }
  return directory;
}

test("PB-F01 extracts module entry and preload assets", () => {
  assert.deepEqual(
    collectHtmlEntryAssets(`
      <script type="module" src="/assets/index-a.js"></script>
      <link rel="modulepreload" href="/assets/runtime-b.js">
      <link rel="icon" href="/icon.svg">
    `),
    ["index-a.js", "runtime-b.js"],
  );
});

test("MDP-001 interaction profile rejects request fan-out", () => {
  const files = Object.fromEntries(
    Array.from({ length: 12 }, (_, index) => [
      `Icon${index}Outlined-a.js`,
      `export const icon${index} = true;`,
    ]),
  );
  const directory = createBundle(files, `<script></script>`);
  try {
    const profile = profileInteractionAssets(
      directory,
      Object.keys(files),
      6100,
      { durationMsMax: 500, assetCountMax: 10, javaScriptCountMax: 0 },
    );
    assert.equal(profile.ok, false);
    assert.deepEqual(profile.gates, {
      durationMs: false,
      assetCount: false,
      javaScriptCount: false,
    });
    assert.equal(profile.classificationCounts.possible_icon_javascript, 12);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("MDP-009 computes the observed nearest-rank P95", () => {
  assert.equal(
    percentile([90, 100, 110, 120, 130, 140, 150, 160, 170, 180], 0.95),
    180,
  );
  assert.equal(percentile([], 0.95), null);
});

test("PB-F01 follows static imports without crossing dynamic imports", () => {
  assert.deepEqual(
    collectStaticImports(
      `import { value } from "./static.js"; import("./lazy.js");`,
    ),
    ["static.js"],
  );
});

test("PB-F01 rejects an eager monolithic Ant Design vendor", () => {
  const directory = createBundle(
    {
      "index-a.js": `import "./antd-vendor-a.js";`,
      "antd-vendor-a.js": "export const value = 1;",
    },
    `<script type="module" src="/assets/index-a.js"></script>`,
  );
  try {
    const profile = profileProductionBundle(directory);
    assert.equal(profile.ok, false);
    assert.equal(profile.gates.noEagerAntDesignVendor, false);
    assert.deepEqual(
      profile.eagerAntDesignVendors.map(({ file }) => file),
      ["antd-vendor-a.js"],
    );
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("PB-F01 accepts a route-local Ant Design dependency", () => {
  const directory = createBundle(
    {
      "index-a.js": `export const load = () => import("./antd-vendor-a.js");`,
      "antd-vendor-a.js": "export const value = 1;",
    },
    `<script type="module" src="/assets/index-a.js"></script>`,
  );
  try {
    const profile = profileProductionBundle(directory);
    assert.equal(profile.ok, true);
    assert.equal(profile.gates.noEagerAntDesignVendor, true);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("PB-F01 scenario profile rejects a JavaScript asset above the critical limit", () => {
  const directory = createBundle(
    { "large.js": "x".repeat(1_000) },
    `<script type="module" src="/assets/large.js"></script>`,
  );
  try {
    const profile = profileAssetFiles(directory, ["large.js"], {
      initialGzipBytesMax: 100,
      largestInitialGzipBytesMax: 10,
    });
    assert.equal(profile.ok, false);
    assert.equal(profile.gates.largestInitialJavaScript, false);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
