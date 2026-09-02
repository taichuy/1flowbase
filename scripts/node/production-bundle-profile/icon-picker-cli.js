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
  rebaseStorageStateCookies,
} = require("../page-debug/auth.js");
const { observeAssetDemand, profileInteractionAssets } = require("./core.js");

function optionValue(name, fallback) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

async function main() {
  const repoRoot = path.resolve(__dirname, "../../..");
  const distDirectory = path.resolve(
    optionValue("--dist", path.join(repoRoot, "web/app/dist")),
  );
  const url = optionValue("--url", "http://127.0.0.1:3300/");
  const apiBaseUrl = optionValue("--api-base-url", "http://127.0.0.1:3300");
  const authCookieOrigin = optionValue("--auth-cookie-origin");
  const outputPath = optionValue("--output");
  const cacheState = optionValue("--cache-state", "cold");
  const durationMsMax = Number.parseInt(
    optionValue("--duration-max", "1000"),
    10,
  );
  if (!new Set(["cold", "warm"]).has(cacheState)) {
    throw new Error("--cache-state must be cold or warm");
  }

  const playwright = loadPlaywright(repoRoot);
  const storageStatePath = path.join(
    fs.mkdtempSync(path.join(os.tmpdir(), "icon-picker-session-")),
    "storage-state.json",
  );
  const credentials = loadRootCredentials({ repoRoot });
  const temporarySession = await openTemporaryConsoleSession({
    playwright,
    apiBaseUrl,
    account: credentials.account,
    password: credentials.password,
    storageStatePath,
  });
  if (authCookieOrigin) {
    rebaseStorageStateCookies(storageStatePath, authCookieOrigin);
  }

  const browser = await playwright.chromium.launch(
    buildChromiumLaunchOptions({ headless: true }),
  );
  try {
    const context = await browser.newContext({
      storageState: storageStatePath,
    });
    const page = await context.newPage();
    const demand = observeAssetDemand(page);
    const pageErrors = [];
    page.on("pageerror", (error) => pageErrors.push(error.message));

    if (cacheState === "warm") {
      await page.goto(url, { waitUntil: "networkidle", timeout: 60_000 });
      await page.goto("about:blank");
    }

    await page.goto(url, { waitUntil: "domcontentloaded", timeout: 60_000 });
    await page.locator('[aria-label="Enter design mode"]').waitFor();
    await page.waitForLoadState("networkidle", { timeout: 60_000 });
    await page.locator('[aria-label="Enter design mode"]').click();
    await page.locator('[aria-label="添加菜单"]').click();
    await page.getByText("新增页面", { exact: true }).click();
    const pickerTrigger = page.locator(
      ".frontstage-page-tree-form__icon-select-button:visible",
    );
    const picker = page.locator(".frontstage-page-tree-form__icon-picker");
    await pickerTrigger.waitFor();
    await page.waitForLoadState("networkidle", { timeout: 60_000 });

    if (cacheState === "warm") {
      await pickerTrigger.click();
      await picker.waitFor();
      await page.waitForLoadState("networkidle", { timeout: 60_000 });
      await page.keyboard.press("Escape");
      await picker.waitFor({ state: "hidden" });
    }

    const openBaseline = new Set(demand.requestedAssets);
    const openStartedAt = Date.now();
    await pickerTrigger.click();
    await picker.waitFor();
    const iconButtons = page.locator(".frontstage-page-tree-form__icon-button");
    await iconButtons.first().waitFor();
    const openReadyMs = Date.now() - openStartedAt;
    const visibleIconCount = await iconButtons.count();
    await page.waitForLoadState("networkidle", { timeout: 60_000 });
    const openAssets = [...demand.requestedAssets].filter(
      (asset) => !openBaseline.has(asset),
    );
    const openProfile = profileInteractionAssets(
      distDirectory,
      openAssets,
      openReadyMs,
      {
        durationMsMax,
        assetCountMax: 8,
        javaScriptCountMax: 1,
      },
    );

    const viewport = page.locator(".frontstage-page-tree-form__icon-viewport");
    const scrollBaseline = new Set(demand.requestedAssets);
    await viewport.evaluate((element) => {
      element.scrollTop = element.scrollHeight / 2;
      element.dispatchEvent(new Event("scroll", { bubbles: true }));
    });
    await page.waitForTimeout(250);
    const scrollAssets = [...demand.requestedAssets].filter(
      (asset) => !scrollBaseline.has(asset),
    );
    const scrollProfile = profileInteractionAssets(
      distDirectory,
      scrollAssets,
      0,
      {
        durationMsMax: Number.MAX_SAFE_INTEGER,
        assetCountMax: 6,
        javaScriptCountMax: 0,
      },
    );

    const search = picker.locator('input[type="search"]');
    await search.fill("AccountBookTwoTone");
    const twoToneButton = page.locator(
      '.frontstage-page-tree-form__icon-button[aria-label="AccountBookTwoTone"]',
    );
    await twoToneButton.waitFor();
    const canonicalName = await twoToneButton.getAttribute("aria-label");
    await twoToneButton.click();
    await picker.waitFor({ state: "hidden" });
    await page.waitForTimeout(100);
    const runtimeIcon = pickerTrigger.locator("svg");
    const runtimePathCount = await runtimeIcon.locator("path").count();

    const result = {
      schemaVersion: "1flowbase.icon-picker-interaction/v1",
      url,
      cacheState,
      openReadyMs,
      visibleIconCount,
      open: openProfile,
      scroll: scrollProfile,
      canonicalName,
      runtimePathCount,
      failedAssets: demand.failedAssets,
      pageErrors,
    };
    result.ok =
      openProfile.ok &&
      scrollProfile.ok &&
      canonicalName === "AccountBookTwoTone" &&
      runtimePathCount >= 2 &&
      demand.failedAssets.length === 0 &&
      pageErrors.length === 0;

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
        await temporarySession.dispose();
      } finally {
        fs.rmSync(path.dirname(storageStatePath), {
          recursive: true,
          force: true,
        });
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
