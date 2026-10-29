#!/usr/bin/env node
"use strict";
// Fixture only. Run against services already started by the assembly/QA owner.
const fs = require("node:fs/promises");
const fsSync = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const { createRequire } = require("node:module");
const {
  loadRootCredentials,
  openTemporaryOwnerSession,
} = require("../../auth.js");
const { runBrowserScene } = require("./browser.cjs");
const { prepareReferences } = require("./visual-references.cjs");

const repoRoot = path.resolve(__dirname, "../../../../..");
function parseArgs(argv) {
  const options = {
    webBaseUrl: "http://127.0.0.1:3100",
    apiBaseUrl: "http://127.0.0.1:7800",
    outputDir: null,
    chrome: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH || null,
    headless: true,
  };
  const keys = {
    "--web-base-url": "webBaseUrl",
    "--api-base-url": "apiBaseUrl",
    "--output-dir": "outputDir",
    "--chrome": "chrome",
    "--reference-manifest": "referenceManifest",
  };
  for (let index = 0; index < argv.length; index++) {
    const arg = argv[index];
    if (arg === "--help" || arg === "-h") return { help: true };
    if (arg === "--headed") {
      options.headless = false;
      continue;
    }
    assert.ok(
      keys[arg] && argv[index + 1] && !argv[index + 1].startsWith("--"),
      `Unknown or incomplete argument: ${arg}`,
    );
    options[keys[arg]] = argv[++index];
  }
  options.webBaseUrl = new URL(options.webBaseUrl).origin;
  options.apiBaseUrl = new URL(options.apiBaseUrl).origin;
  options.outputDir = path.resolve(
    options.outputDir ||
      path.join(
        repoRoot,
        "tmp/test-governance/bounded-containers-browser",
        new Date().toISOString().replaceAll(":", "-"),
      ),
  );
  options.chrome ||= [
    "/usr/bin/google-chrome",
    "/usr/bin/google-chrome-stable",
    "/opt/google/chrome/chrome",
  ].find((candidate) => fsSync.existsSync(candidate));
  assert.ok(
    options.chrome && fsSync.existsSync(options.chrome),
    "Installed Chrome required; set --chrome or PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH",
  );
  return options;
}
function browserCookies(cookie, webBaseUrl) {
  return cookie
    .split(";")
    .map((item) => item.trim())
    .filter(Boolean)
    .map((item) => {
      const separator = item.indexOf("=");
      assert.ok(separator > 0, "Invalid session cookie returned by auth owner");
      return {
        name: item.slice(0, separator),
        value: item.slice(separator + 1),
        url: webBaseUrl,
        httpOnly: true,
        sameSite: "Lax",
        secure: webBaseUrl.startsWith("https:"),
      };
    });
}

async function main(argv = process.argv.slice(2)) {
  const options = parseArgs(argv);
  if (options.help) {
    process.stdout.write(
      "node scripts/node/page-debug/scenarios/bounded-containers/run.cjs [--web-base-url URL] [--api-base-url URL] [--output-dir EMPTY_DIR] [--chrome PATH] --reference-manifest PRIVATE_JSON [--headed]\n",
    );
    return;
  }
  const { webBaseUrl, apiBaseUrl, outputDir } = options;
  await fs.mkdir(outputDir, { recursive: true, mode: 0o700 });
  assert.equal(
    (await fs.readdir(outputDir)).length,
    0,
    "Evidence directory must be empty to avoid mixing candidates",
  );
  const write = (name, data) =>
    fs.writeFile(
      path.join(outputDir, name),
      `${JSON.stringify(data, null, 2)}\n`,
      { mode: 0o600 },
    );
  const receipt = {
    status: "running",
    phase: "authentication",
    started_at: new Date().toISOString(),
    source_sha: execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: repoRoot,
      encoding: "utf8",
      maxBuffer: 1024,
    }).trim(),
    web_base_url: webBaseUrl,
    api_base_url: apiBaseUrl,
    application_name: `BC Browser ${crypto.randomUUID()}`,
    checks_not_claimed: [
      "failure-policy behavior",
      "crash recovery",
      "concurrency races",
      "mobile canvas editing",
    ],
  };
  receipt.visual_references = await prepareReferences(
    options.referenceManifest,
    outputDir,
  );
  let owner, browser, applicationId, failure;
  const cleanup = {
    application_deleted: false,
    session_revoked: false,
    credential_files_created: false,
  };
  const playwright = createRequire(path.join(repoRoot, "web/package.json"))(
    "playwright",
  );
  async function api(method, route, data) {
    const response = await fetch(new URL(route, apiBaseUrl), {
      method,
      signal: AbortSignal.timeout(90000),
      headers: {
        cookie: owner.cookie,
        "x-csrf-token": owner.csrfToken,
        "content-type": "application/json",
      },
      ...(data === undefined ? {} : { body: JSON.stringify(data) }),
    });
    if (!response.ok)
      throw new Error(`${method} ${route} returned HTTP ${response.status}`);
    if (response.status === 204) return null;
    const payload = await response.json();
    assert.ok(
      Object.hasOwn(payload, "data"),
      `${method} ${route} missing API data envelope`,
    );
    return payload.data;
  }
  try {
    const credentials = loadRootCredentials({ repoRoot });
    try {
      owner = await openTemporaryOwnerSession({
        apiBaseUrl,
        ...credentials,
        fetchImpl: (url, init) =>
          fetch(url, { ...init, signal: AbortSignal.timeout(30000) }),
      });
    } catch {
      throw new Error(
        "Temporary session setup failed; verify bootstrap credentials locally",
      );
    }
    receipt.phase = "application-preparation";
    const application = await api("POST", "/api/console/applications", {
      application_type: "workflow",
      workflow_trigger_type: "schedule",
      workflow_trigger_config: {
        cron: "0 0 1 1 *",
        timezone: "UTC",
        input_payload: { items_json: "[1,2,3]" },
      },
      name: receipt.application_name,
      description: "Temporary bounded-container browser acceptance fixture",
      icon: null,
      icon_type: null,
      icon_background: null,
    });
    applicationId = application.id;
    assert.ok(typeof applicationId === "string" && applicationId.length > 0);
    receipt.application_id = applicationId;
    await write("receipt.json", receipt);
    // No publication and no live schedule dispatch: only the formal Test Run action executes it.
    await api(
      "PUT",
      `/api/console/applications/${applicationId}/workflow-schedule-trigger`,
      {
        enabled: false,
        cron: "0 0 1 1 *",
        timezone: "UTC",
        input_payload: { items_json: "[1,2,3]" },
      },
    );
    receipt.phase = "browser-interactions";
    await write("receipt.json", receipt);
    browser = await playwright.chromium.launch({
      executablePath: options.chrome,
      headless: options.headless,
    });
    receipt.chrome_version = browser.version();
    await runBrowserScene({
      browser,
      api,
      applicationId,
      webBaseUrl,
      outputDir,
      cookies: browserCookies(owner.cookie, webBaseUrl),
      write,
      receipt,
    });
    receipt.status = "passed";
  } catch (error) {
    failure = error;
    receipt.status = "failed";
    receipt.error = {
      phase: receipt.phase,
      message: error.message.slice(0, 1200),
    };
  } finally {
    const cleanupErrors = [];
    if (browser)
      try {
        await browser.close();
      } catch {
        cleanupErrors.push("browser-close");
      }
    if (owner && applicationId) {
      try {
        if (receipt.run_id && receipt.status !== "passed") {
          // A failed assertion may leave an in-flight test run. A terminal-run conflict is harmless.
          await api(
            "POST",
            `/api/console/applications/${applicationId}/orchestration/runs/${receipt.run_id}/cancel`,
          ).catch(() => undefined);
        }
        await api("DELETE", `/api/console/applications/${applicationId}`);
        cleanup.application_deleted = true;
      } catch {
        cleanupErrors.push("application-delete");
      }
    }
    if (owner) {
      try {
        await owner.dispose();
        const probe = await fetch(new URL("/api/console/session", apiBaseUrl), {
          headers: { cookie: owner.cookie },
          signal: AbortSignal.timeout(15000),
        });
        cleanup.revoked_session_status = probe.status;
        assert.equal(probe.status, 401);
        cleanup.session_revoked = true;
      } catch {
        cleanupErrors.push("temporary-session-revoke");
      }
    }
    cleanup.errors = cleanupErrors;
    receipt.finished_at = new Date().toISOString();
    if (cleanupErrors.length) {
      receipt.status = "failed";
      failure ||= new Error(`Cleanup failed: ${cleanupErrors.join(", ")}`);
    }
    await write("cleanup.json", cleanup);
    await write("receipt.json", receipt);
  }
  process.stdout.write(
    `${JSON.stringify({ status: receipt.status, outputDir, application_id: applicationId, run_id: receipt.run_id, cleanup })}\n`,
  );
  if (failure) throw failure;
}
if (require.main === module)
  main().catch((error) => {
    process.stderr.write(`[bounded-containers] ${error.message}\n`);
    process.exitCode = 1;
  });
module.exports = { main, parseArgs };
