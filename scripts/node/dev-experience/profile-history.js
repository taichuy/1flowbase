const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { resolveOutputDir } = require("../testing/warning-capture.js");

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function median(values) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2
    ? sorted[middle]
    : (sorted[middle - 1] + sorted[middle]) / 2;
}

function robustLimit(history, multiplier = 3) {
  if (history.length < 5) return null;
  const center = median(history);
  const deviation = median(history.map((value) => Math.abs(value - center)));
  return center + multiplier * 1.4826 * deviation;
}

function evaluateBudget({ value, absoluteMax, history = [] }) {
  const regressionMax = robustLimit(history);
  return {
    ok:
      value <= absoluteMax &&
      (regressionMax === null || value <= regressionMax),
    absoluteMax,
    regressionMax,
    historyStatus: regressionMax === null ? "warming" : "active",
  };
}

function profileHistoryKey(scenario, phase, metric) {
  return `${scenario}\0${phase}\0${metric}`;
}

function attachProfileGates(profile, budgets, scenario, history = {}) {
  return {
    ...profile,
    gates: {
      modules: evaluateBudget({
        value: profile.moduleRequestCount || 0,
        absoluteMax: budgets.cold_module_requests_max,
        history:
          history[
            profileHistoryKey(scenario, profile.phase, "moduleRequestCount")
          ] || [],
      }),
      decodedBytes: evaluateBudget({
        value: profile.decodedBytes || 0,
        absoluteMax: budgets.decoded_bytes_max,
        history:
          history[profileHistoryKey(scenario, profile.phase, "decodedBytes")] ||
          [],
      }),
      failures: evaluateBudget({
        value: profile.failedRequests.length,
        absoluteMax: budgets.failed_modules_max,
        history:
          history[
            profileHistoryKey(scenario, profile.phase, "failedRequests")
          ] || [],
      }),
      runtimeErrors: evaluateBudget({
        value:
          (profile.consoleErrors?.length || 0) +
          (profile.pageErrors?.length || 0),
        absoluteMax: budgets.runtime_errors_max,
        history:
          history[
            profileHistoryKey(scenario, profile.phase, "runtimeErrors")
          ] || [],
      }),
      duration: evaluateBudget({
        value: profile.readyDurationMs ?? profile.durationMs ?? 0,
        absoluteMax: budgets.duration_ms_max ?? Number.MAX_SAFE_INTEGER,
        history:
          history[
            profileHistoryKey(scenario, profile.phase, "readyDurationMs")
          ] || [],
      }),
      pending: evaluateBudget({
        value: profile.pendingRequests?.length || 0,
        absoluteMax: budgets.pending_modules_max ?? 0,
        history:
          history[
            profileHistoryKey(scenario, profile.phase, "pendingRequests")
          ] || [],
      }),
    },
  };
}

function writeReport({ repoRoot, result, env = process.env }) {
  const outputDir = resolveOutputDir(repoRoot, env);
  fs.mkdirSync(outputDir, { recursive: true });
  const reportPath = path.join(outputDir, "dev-experience-profile.json");
  fs.writeFileSync(reportPath, `${JSON.stringify(result, null, 2)}\n`, "utf8");
  return reportPath;
}

function buildReferenceIdentity(
  repoRoot,
  manifestPath,
  { webBaseUrl, publicWebBaseUrl },
) {
  const identityHash = crypto.createHash("sha256");
  for (const filePath of [
    manifestPath,
    path.join(repoRoot, "web", "app", "vite.config.ts"),
  ]) {
    identityHash.update(fs.readFileSync(filePath));
  }
  identityHash.update(
    JSON.stringify({
      node: process.version,
      platform: process.platform,
      arch: process.arch,
      webBaseUrl,
      publicWebBaseUrl,
    }),
  );
  return identityHash.digest("hex");
}

function readHistory({ repoRoot, referenceIdentity, env = process.env }) {
  const reportPath = path.join(
    resolveOutputDir(repoRoot, env),
    "dev-experience-profile.json",
  );
  if (!fs.existsSync(reportPath)) return {};
  try {
    const report = readJson(reportPath);
    return report.referenceIdentity === referenceIdentity
      ? report.history || {}
      : {};
  } catch (_error) {
    return {};
  }
}

function updateHistory(history, profiles) {
  const next = Object.fromEntries(
    Object.entries(history).map(([key, values]) => [key, values.slice(-19)]),
  );
  for (const profile of profiles) {
    const metrics = {
      moduleRequestCount: profile.moduleRequestCount || 0,
      decodedBytes: profile.decodedBytes || 0,
      durationMs: profile.durationMs || 0,
      readyDurationMs: profile.readyDurationMs ?? profile.durationMs ?? 0,
      pendingRequests: profile.pendingRequests?.length || 0,
      failedRequests: profile.failedRequests.length,
      runtimeErrors:
        (profile.consoleErrors?.length || 0) +
        (profile.pageErrors?.length || 0),
    };
    for (const [metric, value] of Object.entries(metrics)) {
      const key = profileHistoryKey(profile.scenario, profile.phase, metric);
      next[key] = [...(next[key] || []), value].slice(-20);
    }
  }
  return next;
}

module.exports = { readJson, median, robustLimit, evaluateBudget, attachProfileGates, writeReport, buildReferenceIdentity, readHistory, updateHistory };
