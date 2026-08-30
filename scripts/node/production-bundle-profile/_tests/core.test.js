const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

const {
  collectHtmlEntryAssets,
  collectStaticImports,
  profileAssetFiles,
  profileProductionBundle,
} = require("../core.js");

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
