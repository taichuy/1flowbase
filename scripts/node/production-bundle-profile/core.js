const fs = require("node:fs");
const path = require("node:path");
const zlib = require("node:zlib");

const DEFAULT_BUDGET = Object.freeze({
  initialGzipBytesMax: 350 * 1024,
  largestInitialGzipBytesMax: 200 * 1024,
  javaScriptCountMax: Number.POSITIVE_INFINITY,
});

const DEFAULT_INTERACTION_BUDGET = Object.freeze({
  durationMsMax: Number.POSITIVE_INFINITY,
  assetCountMax: Number.POSITIVE_INFINITY,
  javaScriptCountMax: Number.POSITIVE_INFINITY,
});

const LIFECYCLE_STAGE_NAMES = Object.freeze([
  "ShellReady",
  "RouteDataReady",
  "CanvasVisible",
  "CanvasInteractive",
  "BackgroundWarmupComplete",
]);

function summarizeLifecycleStages(marks, responseStart = 0) {
  const normalizedMarks = Object.fromEntries(
    LIFECYCLE_STAGE_NAMES.filter((name) => Number.isFinite(marks[name])).map(
      (name) => [name, Math.round(marks[name])],
    ),
  );
  const duration = (from, to) =>
    Number.isFinite(from) && Number.isFinite(to)
      ? Math.max(0, Math.round(to - from))
      : null;

  return {
    marks: normalizedMarks,
    stages: {
      responseToShellReady: duration(responseStart, normalizedMarks.ShellReady),
      shellToRouteDataReady: duration(
        normalizedMarks.ShellReady,
        normalizedMarks.RouteDataReady,
      ),
      routeDataToCanvasVisible: duration(
        normalizedMarks.RouteDataReady,
        normalizedMarks.CanvasVisible,
      ),
      canvasVisibleToInteractive: duration(
        normalizedMarks.CanvasVisible,
        normalizedMarks.CanvasInteractive,
      ),
      interactiveToBackgroundWarmupComplete: duration(
        normalizedMarks.CanvasInteractive,
        normalizedMarks.BackgroundWarmupComplete,
      ),
    },
    complete: LIFECYCLE_STAGE_NAMES.every((name) =>
      Number.isFinite(normalizedMarks[name]),
    ),
  };
}

function normalizeAssetPath(value) {
  return value.replace(/^\.?\//u, "").replace(/^assets\//u, "");
}

function assetNameFromUrl(urlValue) {
  const pathname = new URL(urlValue).pathname;
  for (const directory of ["assets", "icons"]) {
    const marker = `/${directory}/`;
    const index = pathname.indexOf(marker);
    if (index >= 0)
      return `${directory}/${pathname.slice(index + marker.length)}`;
  }
  return null;
}

function observeAssetDemand(page) {
  const requestedAssets = new Set();
  const failedAssets = [];

  page.on("request", (request) => {
    const asset = assetNameFromUrl(request.url());
    if (asset) requestedAssets.add(asset);
  });
  page.on("response", (response) => {
    const asset = assetNameFromUrl(response.url());
    if (asset && response.status() >= 400) {
      failedAssets.push({ asset, status: response.status() });
    }
  });
  page.on("requestfailed", (request) => {
    const asset = assetNameFromUrl(request.url());
    if (asset)
      failedAssets.push({ asset, error: request.failure()?.errorText });
  });

  return { requestedAssets, failedAssets };
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
  // Legacy fixture/bundle fallback only. Current builds provide compiler edges.
  // The already-installed Vite/Oxc parser distinguishes imports from comments,
  // regexes and declaration/template strings; a regular expression cannot.
  const { createRequire } = require("node:module");
  const { parseSync } = createRequire(
    path.resolve(__dirname, "../../../web/package.json"),
  )("vite");
  const parsed = parseSync("bundle.js", source);
  const imports = new Set();
  for (const statement of parsed.program.body) {
    if (
      statement.type !== "ImportDeclaration" &&
      statement.type !== "ExportNamedDeclaration" &&
      statement.type !== "ExportAllDeclaration"
    ) {
      continue;
    }
    const specifier = statement.source?.value;
    if (typeof specifier === "string" && specifier.startsWith(".")) {
      imports.add(normalizeAssetPath(specifier));
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
  const manifestPath = path.join(distDirectory, "scenario-chunk-manifest.json");
  const manifest = fs.existsSync(manifestPath) ? JSON.parse(fs.readFileSync(manifestPath, "utf8")) : null;
  const graph = manifest?.version === 2
    ? new Map(manifest.chunks.map((chunk) => [normalizeAssetPath(chunk.file), chunk]))
    : null;
  const visited = new Set();

  while (pending.length > 0) {
    const asset = pending.pop();
    if (!asset || visited.has(asset)) continue;
    const absolutePath = path.join(assetsDirectory, asset);
    if (!fs.existsSync(absolutePath)) continue;
    visited.add(asset);
    if (!asset.endsWith(".js")) continue;
    if (graph) {
      const chunk = graph.get(asset);
      if (!chunk) throw new Error(`Compiler graph is missing ${asset}`);
      pending.push(...[...chunk.imports, ...chunk.css].map(normalizeAssetPath));
    } else {
      const source = fs.readFileSync(absolutePath, "utf8");
      for (const specifier of collectStaticImports(source)) {
        pending.push(resolveImportedAsset(asset, specifier));
      }
    }
  }

  return [...visited].sort();
}

function profileAssetFiles(distDirectory, files, budget = DEFAULT_BUDGET) {
  const assetsDirectory = path.join(distDirectory, "assets");
  const initialAssets = [...new Set(files)].sort().map((file) => {
    const normalizedFile = normalizeAssetPath(file);
    const contents = fs.readFileSync(
      file.startsWith("icons/")
        ? path.join(distDirectory, file)
        : path.join(assetsDirectory, normalizedFile),
    );
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
    javaScriptCount:
      initialJavaScript.length <=
      (budget.javaScriptCountMax ?? Number.POSITIVE_INFINITY),
  };

  return {
    budget,
    gates,
    ok: Object.values(gates).every(Boolean),
    initialAssetCount: initialAssets.length,
    initialJavaScriptCount: initialJavaScript.length,
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

function profileScenarioBundle(distDirectory, scene) {
  const manifest = JSON.parse(fs.readFileSync(path.join(distDirectory, "scenario-chunk-manifest.json"), "utf8"));
  if (manifest.version !== 2 || !manifest.scenes?.[scene]?.roots?.length) {
    throw new Error(`Missing compiler-owned scene: ${scene}`);
  }
  const chunks = new Map(manifest.chunks.map((chunk) => [chunk.file, chunk]));
  const files = new Set(manifest.scenes[scene].staticClosure);
  for (const file of [...files]) for (const css of chunks.get(file)?.css ?? []) files.add(css);
  return {
    scene,
    coverage: "Compiler static closure and CSS only; dynamic/Worker/data demands require the real browser scene trace",
    roots: manifest.scenes[scene].roots,
    dynamicBoundaries: [...new Set(manifest.scenes[scene].staticClosure.flatMap((file) => chunks.get(file)?.dynamicImports ?? []))].sort(),
    ...profileAssetFiles(distDirectory, [...files], {
      initialGzipBytesMax: Infinity,
      largestInitialGzipBytesMax: Infinity,
      javaScriptCountMax: Infinity,
    }),
  };
}

function classifyAsset(file) {
  if (
    /^(?:(?:assets|icons)\/)?page-tree-icons-pack-[a-f0-9]{12}-.+\.svg$/u.test(
      file,
    )
  ) {
    return "page_tree_icon_preview";
  }
  if (/page-tree-icon-component-pack/u.test(file)) {
    return "page_tree_icon_runtime";
  }
  if (/Ant|Outlined|Filled|TwoTone/u.test(file) && file.endsWith(".js")) {
    return "possible_icon_javascript";
  }
  if (file.endsWith(".js")) return "javascript";
  if (file.endsWith(".css")) return "stylesheet";
  return "asset";
}

function profileInteractionAssets(
  distDirectory,
  files,
  durationMs,
  budget = DEFAULT_INTERACTION_BUDGET,
) {
  const assets = profileAssetFiles(distDirectory, files, {
    initialGzipBytesMax: Number.POSITIVE_INFINITY,
    largestInitialGzipBytesMax: Number.POSITIVE_INFINITY,
  }).initialAssets.map((asset) => ({
    ...asset,
    classification: classifyAsset(asset.file),
  }));
  const javaScriptCount = assets.filter(({ file }) =>
    file.endsWith(".js"),
  ).length;
  const gates = {
    durationMs: durationMs <= budget.durationMsMax,
    assetCount: assets.length <= budget.assetCountMax,
    javaScriptCount: javaScriptCount <= budget.javaScriptCountMax,
  };
  return {
    budget,
    durationMs,
    assetCount: assets.length,
    javaScriptCount,
    classificationCounts: Object.fromEntries(
      [...new Set(assets.map(({ classification }) => classification))].map(
        (classification) => [
          classification,
          assets.filter((asset) => asset.classification === classification)
            .length,
        ],
      ),
    ),
    assets,
    gates,
    ok: Object.values(gates).every(Boolean),
  };
}

function percentile(values, quantile) {
  if (values.length === 0) return null;
  if (!Number.isFinite(quantile) || quantile < 0 || quantile > 1) {
    throw new Error("Quantile must be between zero and one");
  }
  const ordered = [...values].sort((left, right) => left - right);
  const rank = Math.max(0, Math.ceil(quantile * ordered.length) - 1);
  return ordered[rank];
}

module.exports = {
  DEFAULT_BUDGET,
  DEFAULT_INTERACTION_BUDGET,
  assetNameFromUrl,
  classifyAsset,
  collectHtmlEntryAssets,
  collectStaticImports,
  profileScenarioBundle,
  observeAssetDemand,
  percentile,
  profileAssetFiles,
  profileInteractionAssets,
  profileProductionBundle,
  summarizeLifecycleStages,
};
