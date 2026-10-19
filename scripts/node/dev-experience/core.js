const fs = require("node:fs");
const path = require("node:path");
const { getRepoRoot, resolveOutputDir } = require("../testing/warning-capture.js");
const { isActionableConsoleError, summarizeResources, profileScenario, profileHmrTransport, triggerRuntimeRecovery } = require("./browser-profile.js");
const { DEFAULT_WEB_BASE_URL } = require("./profile-config.js");
const { median, robustLimit, evaluateBudget, attachProfileGates, readHistory, updateHistory } = require("./profile-history.js");
const { resolveScenarioFixtures } = require("./scenario-fixtures.js");
const { runSmoke } = require("./smoke-runner.js");
const { buildSourceGraph, stronglyConnectedComponents, buildCondensation, reachableComponentSets, cacheIdentity, parseOptimizeDepsInclude, analyzeStatic } = require("./static-analysis.js");
const { createIsolatedViteCacheDirectory, startIsolatedVite, stopIsolatedVite } = require("./vite-runtime.js");

function parseCliArgs(argv) {
  const options = {
    smoke: false,
    webBaseUrl: DEFAULT_WEB_BASE_URL,
    publicWebBaseUrl: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--smoke") options.smoke = true;
    else if (argv[index] === "--web-base-url" && argv[index + 1])
      options.webBaseUrl = argv[++index];
    else if (argv[index] === "--public-web-base-url" && argv[index + 1])
      options.publicWebBaseUrl = argv[++index];
    else if (argv[index] === "--help" || argv[index] === "-h")
      options.help = true;
    else throw new Error(`Unknown dev-experience option: ${argv[index]}`);
  }
  return options;
}

async function main(argv = [], deps = {}) {
  const options = parseCliArgs(argv);
  const writeStdout =
    deps.writeStdout || ((text) => process.stdout.write(text));
  if (options.help) {
    writeStdout(
      "Usage: node scripts/node/dev-experience/cli.js [--smoke] [--web-base-url <url>] [--public-web-base-url <url>]\n",
    );
    return 0;
  }
  const staticResult = analyzeStatic({
    repoRoot: deps.repoRoot || getRepoRoot(),
  });
  writeStdout(
    `[dev-experience] static ${staticResult.ok ? "passed" : "failed"}: ${staticResult.graph.vertices} modules, ${staticResult.graph.edges} edges\n`,
  );
  const outputDir = resolveOutputDir(deps.repoRoot || getRepoRoot(), deps.env || process.env);
  fs.mkdirSync(outputDir, { recursive: true });
  fs.writeFileSync(
    path.join(outputDir, "dev-experience-static.json"),
    `${JSON.stringify(staticResult, null, 2)}\n`,
  );
  for (const finding of staticResult.findings) {
    writeStdout(`[dev-experience:${finding.code}] ${finding.file || "web/app/vite.config.ts"}: ${finding.dependency}\n`);
  }
  if (!staticResult.ok) return 1;
  if (!options.smoke) return 0;
  const smokeResult = await (deps.runSmokeImpl || runSmoke)({
    repoRoot: deps.repoRoot || getRepoRoot(),
    webBaseUrl: options.webBaseUrl,
    publicWebBaseUrl: options.publicWebBaseUrl,
    env: deps.env || process.env,
  });
  return smokeResult.ok ? 0 : 1;
}

module.exports = {
  analyzeStatic,
  buildCondensation,
  buildSourceGraph,
  cacheIdentity,
  createIsolatedViteCacheDirectory,
  attachProfileGates,
  evaluateBudget,
  isActionableConsoleError,
  main,
  median,
  parseOptimizeDepsInclude,
  profileScenario,
  profileHmrTransport,
  reachableComponentSets,
  readHistory,
  robustLimit,
  resolveScenarioFixtures,
  runSmoke,
  startIsolatedVite,
  stopIsolatedVite,
  stronglyConnectedComponents,
  summarizeResources,
  triggerRuntimeRecovery,
  updateHistory,
};
