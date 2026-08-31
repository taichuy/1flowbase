#!/usr/bin/env node

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const {
  buildChromiumLaunchOptions,
  loadPlaywright,
} = require("../page-debug/core.js");
const {
  loadRootCredentials,
  openTemporaryConsoleSession,
} = require("../page-debug/auth.js");
const { profileAssetFiles, profileInteractionAssets } = require("./core.js");

function optionValue(name, fallback) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

function optionValues(name) {
  return process.argv.flatMap((value, index) =>
    value === name && process.argv[index + 1] ? [process.argv[index + 1]] : [],
  );
}

function assetNameFromUrl(urlValue) {
  const pathname = new URL(urlValue).pathname;
  const marker = "/assets/";
  const index = pathname.indexOf(marker);
  return index >= 0 ? pathname.slice(index + marker.length) : null;
}

async function main() {
  const repoRoot = path.resolve(__dirname, "../../..");
  const distDirectory = path.resolve(
    optionValue("--dist", path.join(repoRoot, "web/app/dist")),
  );
  const url = optionValue("--url", "http://127.0.0.1:4173/");
  const readySelector = optionValue(
    "--ready-selector",
    '[data-testid="builtin-password-sign-in"]',
  );
  const outputPath = optionValue("--output");
  const interactionSelector = optionValue("--interaction-selector");
  const interactionReadySelector = optionValue(
    "--interaction-ready-selector",
    interactionSelector,
  );
  const authenticated = process.argv.includes("--authenticated");
  const apiBaseUrl = optionValue("--api-base-url", "http://127.0.0.1:7800");
  const budget = {
    initialGzipBytesMax: Number.parseInt(
      optionValue("--initial-gzip-max", String(350 * 1024)),
      10,
    ),
    largestInitialGzipBytesMax: Number.parseInt(
      optionValue("--largest-gzip-max", String(200 * 1024)),
      10,
    ),
  };
  const forbiddenPatterns = optionValues("--forbid-pattern").map(
    (pattern) => new RegExp(pattern, "u"),
  );
  const forbiddenInteractionPatterns = optionValues(
    "--forbid-interaction-pattern",
  ).map((pattern) => new RegExp(pattern, "u"));
  const interactionBudget = {
    durationMsMax: Number.parseInt(
      optionValue(
        "--interaction-duration-max",
        String(Number.MAX_SAFE_INTEGER),
      ),
      10,
    ),
    assetCountMax: Number.parseInt(
      optionValue("--interaction-asset-max", String(Number.MAX_SAFE_INTEGER)),
      10,
    ),
    javaScriptCountMax: Number.parseInt(
      optionValue("--interaction-js-max", String(Number.MAX_SAFE_INTEGER)),
      10,
    ),
  };
  const playwright = loadPlaywright(repoRoot);
  const browser = await playwright.chromium.launch(
    buildChromiumLaunchOptions({ headless: true }),
  );
  const requestedAssets = new Set();
  const failedAssets = [];
  const pageErrors = [];
  let temporarySession = null;
  let storageStatePath = null;
  if (authenticated) {
    storageStatePath = path.join(
      fs.mkdtempSync(path.join(os.tmpdir(), "production-bundle-session-")),
      "storage-state.json",
    );
    const credentials = loadRootCredentials({ repoRoot });
    temporarySession = await openTemporaryConsoleSession({
      playwright,
      apiBaseUrl,
      account: credentials.account,
      password: credentials.password,
      storageStatePath,
    });
  }
  const context = await browser.newContext(
    storageStatePath ? { storageState: storageStatePath } : {},
  );
  const page = await context.newPage();
  page.on("response", (response) => {
    const asset = assetNameFromUrl(response.url());
    if (!asset) return;
    if (response.status() >= 400) {
      failedAssets.push({ asset, status: response.status() });
    } else {
      requestedAssets.add(asset);
    }
  });
  page.on("requestfailed", (request) => {
    const asset = assetNameFromUrl(request.url());
    if (asset)
      failedAssets.push({ asset, error: request.failure()?.errorText });
  });
  page.on("pageerror", (error) => pageErrors.push(error.message));

  try {
    const startedAt = Date.now();
    await page.goto(url, { waitUntil: "domcontentloaded", timeout: 60_000 });
    await page.waitForSelector(readySelector, {
      state: "visible",
      timeout: 60_000,
    });
    const readyDurationMs = Date.now() - startedAt;
    const readyAssets = [...requestedAssets];
    await page
      .waitForLoadState("networkidle", { timeout: 5_000 })
      .catch(() => {});
    const settledAssets = [...requestedAssets];
    const readyProfile = profileAssetFiles(distDirectory, readyAssets, budget);
    const settledProfile = profileAssetFiles(
      distDirectory,
      settledAssets,
      budget,
    );
    const unexpectedReadyAssets = readyProfile.initialAssets
      .map(({ file }) => file)
      .filter((file) =>
        forbiddenPatterns.some((pattern) => pattern.test(file)),
      );
    const unexpectedSettledAssets = settledProfile.initialAssets
      .map(({ file }) => file)
      .filter((file) =>
        forbiddenPatterns.some((pattern) => pattern.test(file)),
      );
    let interaction = null;
    if (interactionSelector) {
      const baselineAssets = new Set(requestedAssets);
      const interactionStartedAt = Date.now();
      await page.click(interactionSelector);
      if (interactionReadySelector) {
        await page.waitForSelector(interactionReadySelector, {
          state: "visible",
          timeout: 60_000,
        });
      }
      const readyMs = Date.now() - interactionStartedAt;
      const readyFiles = [...requestedAssets].filter(
        (asset) => !baselineAssets.has(asset),
      );
      await page
        .waitForLoadState("networkidle", { timeout: 5_000 })
        .catch(() => {});
      const settledMs = Date.now() - interactionStartedAt;
      const settledFiles = [...requestedAssets].filter(
        (asset) => !baselineAssets.has(asset),
      );
      const unexpectedAssets = settledFiles.filter((file) =>
        forbiddenInteractionPatterns.some((pattern) => pattern.test(file)),
      );
      const readyInteractionProfile = profileInteractionAssets(
        distDirectory,
        readyFiles,
        readyMs,
        interactionBudget,
      );
      const settledInteractionProfile = profileInteractionAssets(
        distDirectory,
        settledFiles,
        readyMs,
        interactionBudget,
      );
      interaction = {
        selector: interactionSelector,
        readySelector: interactionReadySelector,
        readyMs,
        settledMs,
        forbiddenPatterns: forbiddenInteractionPatterns.map(
          (pattern) => pattern.source,
        ),
        unexpectedAssets,
        ready: readyInteractionProfile,
        settled: settledInteractionProfile,
        ok:
          readyInteractionProfile.ok &&
          settledInteractionProfile.ok &&
          unexpectedAssets.length === 0,
      };
    }
    const result = {
      url,
      finalUrl: page.url(),
      authenticated,
      readySelector,
      readyDurationMs,
      durationMs: Date.now() - startedAt,
      forbiddenPatterns: forbiddenPatterns.map((pattern) => pattern.source),
      unexpectedReadyAssets,
      unexpectedSettledAssets,
      ready: readyProfile,
      settled: settledProfile,
      failedAssets,
      pageErrors,
      interaction,
    };
    result.ok =
      result.ready.ok &&
      unexpectedSettledAssets.length === 0 &&
      failedAssets.length === 0 &&
      pageErrors.length === 0 &&
      (interaction?.ok ?? true);
    const receipt = `${JSON.stringify(result, null, 2)}\n`;
    if (outputPath) {
      fs.mkdirSync(path.dirname(path.resolve(outputPath)), { recursive: true });
      fs.writeFileSync(path.resolve(outputPath), receipt);
    }
    process.stdout.write(receipt);
    return result.ok ? 0 : 1;
  } finally {
    try {
      await browser.close();
    } finally {
      try {
        await temporarySession?.dispose();
      } finally {
        if (storageStatePath) {
          fs.rmSync(path.dirname(storageStatePath), {
            recursive: true,
            force: true,
          });
        }
      }
    }
  }
}

main().then(
  (exitCode) => {
    process.exitCode = exitCode;
  },
  (error) => {
    process.stderr.write(`${error.stack || error}\n`);
    process.exitCode = 1;
  },
);
