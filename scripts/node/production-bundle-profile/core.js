const fs = require("node:fs");
const path = require("node:path");
const zlib = require("node:zlib");

const DEFAULT_BUDGET = Object.freeze({
  initialGzipBytesMax: 350 * 1024,
  largestInitialGzipBytesMax: 200 * 1024,
});

function normalizeAssetPath(value) {
  return value.replace(/^\.?\//u, "").replace(/^assets\//u, "");
}

function collectHtmlEntryAssets(html) {
  const assets = new Set();
  const entryPattern =
    /<(?:script|link)\b[^>]*(?:src|href)=["']([^"']+)["'][^>]*>/gu;
  let match = entryPattern.exec(html);
  while (match) {
    if (/\/assets\//u.test(match[1])) {
      assets.add(normalizeAssetPath(match[1]));
    }
    match = entryPattern.exec(html);
  }
  return [...assets];
}

function collectStaticImports(source) {
  const imports = new Set();
  const patterns = [
    /\b(?:import|export)\s*[\s\S]*?\sfrom\s*["']([^"']+)["']/gu,
    /\bimport\s*["']([^"']+)["']/gu,
  ];
  for (const pattern of patterns) {
    let match = pattern.exec(source);
    while (match) {
      if (match[1].startsWith(".")) imports.add(normalizeAssetPath(match[1]));
      match = pattern.exec(source);
    }
  }
  return [...imports];
}

function resolveImportedAsset(importer, specifier) {
  return normalizeAssetPath(
    path.posix.normalize(
      path.posix.join(path.posix.dirname(importer), specifier),
    ),
  );
}

function collectInitialAssetClosure(distDirectory) {
  const assetsDirectory = path.join(distDirectory, "assets");
  const html = fs.readFileSync(path.join(distDirectory, "index.html"), "utf8");
  const pending = collectHtmlEntryAssets(html);
  const visited = new Set();

  while (pending.length > 0) {
    const asset = pending.pop();
    if (!asset || visited.has(asset)) continue;
    const absolutePath = path.join(assetsDirectory, asset);
    if (!fs.existsSync(absolutePath)) continue;
    visited.add(asset);
    if (!asset.endsWith(".js")) continue;
    const source = fs.readFileSync(absolutePath, "utf8");
    for (const specifier of collectStaticImports(source)) {
      pending.push(resolveImportedAsset(asset, specifier));
    }
  }

  return [...visited].sort();
}

function profileAssetFiles(distDirectory, files, budget = DEFAULT_BUDGET) {
  const assetsDirectory = path.join(distDirectory, "assets");
  const initialAssets = [...new Set(files)].sort().map((file) => {
    const contents = fs.readFileSync(path.join(assetsDirectory, file));
    return {
      file,
      rawBytes: contents.byteLength,
      gzipBytes: zlib.gzipSync(contents).byteLength,
    };
  });
  const initialJavaScript = initialAssets.filter(({ file }) =>
    file.endsWith(".js"),
  );
  const initialGzipBytes = initialJavaScript.reduce(
    (total, asset) => total + asset.gzipBytes,
    0,
  );
  const largestInitialJavaScript = [...initialJavaScript].sort(
    (left, right) => right.gzipBytes - left.gzipBytes,
  )[0] ?? { file: null, gzipBytes: 0 };
  const eagerAntDesignVendors = initialJavaScript.filter(({ file }) =>
    /^antd-vendor-.+\.js$/u.test(file),
  );
  const gates = {
    noEagerAntDesignVendor: eagerAntDesignVendors.length === 0,
    initialGzipBytes: initialGzipBytes <= budget.initialGzipBytesMax,
    largestInitialJavaScript:
      largestInitialJavaScript.gzipBytes <= budget.largestInitialGzipBytesMax,
  };

  return {
    budget,
    gates,
    ok: Object.values(gates).every(Boolean),
    initialAssetCount: initialAssets.length,
    initialGzipBytes,
    largestInitialJavaScript,
    eagerAntDesignVendors,
    initialAssets,
  };
}

function profileProductionBundle(distDirectory, budget = DEFAULT_BUDGET) {
  return profileAssetFiles(
    distDirectory,
    collectInitialAssetClosure(distDirectory),
    budget,
  );
}

module.exports = {
  DEFAULT_BUDGET,
  collectHtmlEntryAssets,
  collectStaticImports,
  profileAssetFiles,
  profileProductionBundle,
};
