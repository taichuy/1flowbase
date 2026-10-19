const fs = require("node:fs");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { getRepoRoot, resolveOutputDir } = require("../testing/warning-capture.js");
const { buildChromiumLaunchOptions, loadPlaywright } = require("../page-debug/core.js");
const { loadRootCredentials, openTemporaryConsoleSession } = require("../page-debug/auth.js");
const { profileScenario, profileHmrTransport, triggerRuntimeRecovery } = require("./browser-profile.js");
const { DEFAULT_MANIFEST_PATH, DEFAULT_WEB_BASE_URL } = require("./profile-config.js");
const { readJson, attachProfileGates, writeReport, buildReferenceIdentity, readHistory, updateHistory } = require("./profile-history.js");
const { resolveScenarioFixtures } = require("./scenario-fixtures.js");
const { analyzeStatic } = require("./static-analysis.js");
const { startIsolatedVite, stopIsolatedVite, apiBaseUrlForRepo } = require("./vite-runtime.js");

async function runSmoke({
  repoRoot = getRepoRoot(),
  manifestPath = DEFAULT_MANIFEST_PATH,
  webBaseUrl = DEFAULT_WEB_BASE_URL,
  publicWebBaseUrl = null,
  env = process.env,
} = {}) {
  const manifest = readJson(manifestPath);
  const playwright = loadPlaywright(repoRoot);
  const outputDir = resolveOutputDir(repoRoot, env);
  fs.mkdirSync(outputDir, { recursive: true });
  const storageStatePath = path.join(
    outputDir,
    "dev-experience-storage-state.json",
  );
  const apiBaseUrl = apiBaseUrlForRepo(repoRoot);
  const credentials = loadRootCredentials({ repoRoot, sourceEnv: env });
  const temporarySession = await openTemporaryConsoleSession({
    playwright,
    apiBaseUrl,
    account: credentials.account,
    password: credentials.password,
    storageStatePath,
  });
  let scenarios;
  let browser;
  try {
    scenarios = await resolveScenarioFixtures({
      playwright,
      apiBaseUrl,
      storageStatePath,
      scenarios: manifest.scenarios,
    });
    browser = await playwright.chromium.launch(
      buildChromiumLaunchOptions({ headless: true, env }),
    );
  } catch (error) {
    await temporarySession.dispose();
    if (fs.existsSync(storageStatePath)) fs.unlinkSync(storageStatePath);
    throw error;
  }
  const profiles = [];
  const referenceIdentity = buildReferenceIdentity(repoRoot, manifestPath, {
    webBaseUrl,
    publicWebBaseUrl,
  });
  const history = readHistory({ repoRoot, referenceIdentity, env });
  try {
    for (const scenario of scenarios) {
      const scenarioBaseUrl =
        scenario.base_url_kind === "public" ? publicWebBaseUrl : webBaseUrl;
      if (!scenarioBaseUrl) {
        throw new Error(
          `Dev experience scenario '${scenario.id}' requires --public-web-base-url`,
        );
      }
      const targetUrl = `${scenarioBaseUrl.replace(/\/$/u, "")}${scenario.fixture_path}`;
      const context = await browser.newContext(
        scenario.session_kind === "incognito"
          ? {}
          : { storageState: storageStatePath },
      );
      const page = await context.newPage();
      const cold = await profileScenario({
        page,
        phase: "cold",
        url: targetUrl,
        scenario,
      });
      profiles.push({
        scenario: scenario.id,
        ...attachProfileGates(cold, scenario.budgets, scenario.id, history),
      });
      const warm = await profileScenario({
        page,
        phase: "warm",
        url: targetUrl,
        scenario,
      });
      profiles.push({
        scenario: scenario.id,
        ...attachProfileGates(warm, scenario.budgets, scenario.id, history),
      });
      if (scenario.phases.includes("hmr")) {
        const hmr = await profileHmrTransport({ page, webBaseUrl });
        profiles.push({
          scenario: scenario.id,
          ...attachProfileGates(hmr, scenario.budgets, scenario.id, history),
        });
      }
      if (scenario.phases.includes("recovery")) {
        const recoveryTransitions = await triggerRuntimeRecovery({
          page,
          webBaseUrl,
        });
        const recoveryPage = await context.newPage();
        const recovery = await profileScenario({
          page: recoveryPage,
          phase: "recovery",
          url: targetUrl,
          scenario,
        });
        await recoveryPage.close();
        profiles.push({
          scenario: scenario.id,
          recoveryTransitions,
          ...attachProfileGates(
            recovery,
            scenario.budgets,
            scenario.id,
            history,
          ),
        });
      }
      await context.close();

      if (scenario.phases.includes("concurrent")) {
        const contexts = await Promise.all(
          Array.from({ length: 4 }, () =>
            browser.newContext({ storageState: storageStatePath }),
          ),
        );
        const concurrentProfiles = await Promise.all(
          contexts.map(async (candidateContext) => {
            const candidatePage = await candidateContext.newPage();
            return profileScenario({
              page: candidatePage,
              phase: "concurrent",
              url: targetUrl,
              scenario,
            });
          }),
        );
        profiles.push({
          scenario: scenario.id,
          ...attachProfileGates(
            {
              phase: "concurrent",
              readyDurationMs: Math.max(
                ...concurrentProfiles.map((profile) => profile.readyDurationMs),
              ),
              durationMs: Math.max(
                ...concurrentProfiles.map((profile) => profile.durationMs),
              ),
              moduleRequestCount: Math.max(
                ...concurrentProfiles.map(
                  (profile) => profile.moduleRequestCount,
                ),
              ),
              decodedBytes: Math.max(
                ...concurrentProfiles.map((profile) => profile.decodedBytes),
              ),
              failedRequests: concurrentProfiles.flatMap(
                (profile) => profile.failedRequests,
              ),
              consoleErrors: concurrentProfiles.flatMap(
                (profile) => profile.consoleErrors,
              ),
              pageErrors: concurrentProfiles.flatMap(
                (profile) => profile.pageErrors,
              ),
              requestCount: Math.max(
                ...concurrentProfiles.map((profile) => profile.requestCount),
              ),
              transferBytes: Math.max(
                ...concurrentProfiles.map((profile) => profile.transferBytes),
              ),
              transformDurationMs: Math.max(
                ...concurrentProfiles.map(
                  (profile) => profile.transformDurationMs,
                ),
              ),
              finalUrls: concurrentProfiles.map((profile) => profile.finalUrl),
              resources: concurrentProfiles.map((profile) => profile.resources),
            },
            scenario.budgets,
            scenario.id,
            history,
          ),
        });
        await Promise.all(
          contexts.map((candidateContext) => candidateContext.close()),
        );
      }
    }

    if (
      scenarios.some((scenario) => scenario.phases.includes("cache-rebuild"))
    ) {
      const isolated = await startIsolatedVite(repoRoot);
      try {
        for (const scenario of scenarios.filter((candidate) =>
          candidate.phases.includes("cache-rebuild"),
        )) {
          const context = await browser.newContext({
            storageState: storageStatePath,
          });
          const page = await context.newPage();
          let profile;
          try {
            profile = await profileScenario({
              page,
              phase: "cache-rebuild",
              url: `${isolated.baseUrl}${scenario.fixture_path}`,
              scenario,
            });
          } catch (error) {
            throw new Error(
              `${error.message}\n${isolated.log().slice(-4000)}`,
              { cause: error },
            );
          }
          profiles.push({
            scenario: scenario.id,
            cacheIdentity: isolated.readiness.cacheIdentity,
            ...attachProfileGates(
              profile,
              scenario.budgets,
              scenario.id,
              history,
            ),
          });
          await context.close();
        }
      } finally {
        await stopIsolatedVite(isolated);
      }
    }
  } finally {
    try {
      await browser.close();
    } finally {
      await temporarySession.dispose();
      if (fs.existsSync(storageStatePath)) fs.unlinkSync(storageStatePath);
    }
  }
  const staticResult = analyzeStatic({ repoRoot });
  const result = {
    schemaVersion: manifest.schema_version,
    candidateSha: execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: repoRoot,
      encoding: "utf8",
    }).trim(),
    generatedAt: new Date().toISOString(),
    environment: {
      node: process.version,
      platform: process.platform,
      arch: process.arch,
      browser: "chromium",
      webBaseUrl,
      publicWebBaseUrl,
      apiBaseUrl,
    },
    referenceIdentity,
    staticGraph: staticResult.graph,
    ok:
      staticResult.ok &&
      profiles.every((profile) =>
        Object.values(profile.gates).every((gate) => gate.ok),
      ),
    profiles,
    history: updateHistory(history, profiles),
  };
  writeReport({ repoRoot, result, env });
  return result;
}

module.exports = { runSmoke };
