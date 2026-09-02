#!/usr/bin/env node

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const {
  buildChromiumLaunchOptions,
  loadPlaywright,
} = require("../page-debug/core.js");

function optionValue(name, fallback) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

function fingerprint(value) {
  return crypto.createHash("sha256").update(value).digest("hex");
}

async function capture(browser, url) {
  const context = await browser.newContext();
  const page = await context.newPage();
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  try {
    await page.goto(url, { waitUntil: "domcontentloaded", timeout: 60_000 });
    await page.waitForSelector(
      '[data-testid="builtin-password-sign-in"], [data-public-auth-phase="ready"]',
      {
        state: "visible",
        timeout: 60_000,
      },
    );
    const loginInstances = await page.evaluate(async () => {
      const response = await fetch("/api/public/auth/login-instances");
      if (!response.ok) throw new Error(`login instances: ${response.status}`);
      return response.json();
    });
    const publicBlocks = JSON.stringify(
      (loginInstances.items || loginInstances || []).map((item) => ({
        id: item.id,
        public_ui_block: item.public_ui_block,
        public_variables: item.public_variables,
      })),
    );
    const visual = await page.evaluate(() => {
      const button = document.querySelector("button");
      const style = button ? getComputedStyle(button) : null;
      return {
        bodyText: document.body.innerText.replace(/\s+/gu, " ").trim(),
        button: style
          ? {
              backgroundColor: style.backgroundColor,
              borderRadius: style.borderRadius,
              color: style.color,
            }
          : null,
      };
    });
    return {
      url,
      publicUiFingerprint: fingerprint(publicBlocks),
      domFingerprint: fingerprint(visual.bodyText),
      visual,
      pageErrors,
    };
  } finally {
    await context.close();
  }
}

async function main() {
  const repoRoot = path.resolve(__dirname, "../../..");
  const leftUrl = optionValue("--left-url", "http://127.0.0.1:3100/");
  const rightUrl = optionValue("--right-url", "http://127.0.0.1:3300/");
  const outputPath = optionValue("--output");
  const playwright = loadPlaywright(repoRoot);
  const browser = await playwright.chromium.launch(
    buildChromiumLaunchOptions({ headless: true }),
  );
  try {
    const [left, right] = await Promise.all([
      capture(browser, leftUrl),
      capture(browser, rightUrl),
    ]);
    const result = {
      schemaVersion: "1flowbase.public-auth-paired-receipt/v1",
      left,
      right,
      gates: {
        samePublicUi: left.publicUiFingerprint === right.publicUiFingerprint,
        sameDom: left.domFingerprint === right.domFingerprint,
        sameTheme:
          JSON.stringify(left.visual.button) ===
          JSON.stringify(right.visual.button),
        noPageErrors:
          left.pageErrors.length === 0 && right.pageErrors.length === 0,
      },
    };
    result.ok = Object.values(result.gates).every(Boolean);
    const receipt = `${JSON.stringify(result, null, 2)}\n`;
    if (outputPath) {
      fs.mkdirSync(path.dirname(path.resolve(outputPath)), { recursive: true });
      fs.writeFileSync(path.resolve(outputPath), receipt);
    }
    process.stdout.write(receipt);
    return result.ok ? 0 : 1;
  } finally {
    await browser.close();
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
