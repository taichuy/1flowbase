const fs = require('node:fs');
const path = require('node:path');
const { COVERAGE_ROOT, frontendThresholds, backendThresholds } = require('../testing/coverage-thresholds.js');

const FRONTEND_METRICS = ['lines', 'functions', 'statements', 'branches'];

const COVERAGE_SCOPE_LABEL = '1flowbase-verify-coverage';

const BACKEND_COVERAGE_ENTRY_BY_KEY = new Map(backendThresholds.map((entry) => [entry.key, entry]));

function selectBackendCoverageEntries(backendKeys) {
  if (!backendKeys || backendKeys.length === 0) {
    return backendThresholds;
  }

  return backendKeys.map((backendKey) => {
    const entry = BACKEND_COVERAGE_ENTRY_BY_KEY.get(backendKey);

    if (!entry) {
      throw new Error(`Unknown backend coverage package: ${backendKey}`);
    }

    return entry;
  });
}

function normalizeCoveragePath(filePath) {
  return filePath.replace(/\\/gu, '/');
}

function readMetricPct(metricSummary) {
  if (!metricSummary || typeof metricSummary !== 'object') {
    return null;
  }

  if (
    Number.isFinite(metricSummary.total)
    && Number.isFinite(metricSummary.covered)
    && metricSummary.total > 0
  ) {
    return (metricSummary.covered / metricSummary.total) * 100;
  }

  if (Number.isFinite(metricSummary.pct)) {
    return metricSummary.pct;
  }

  return null;
}

function aggregateMetric(matchedEntries, metric) {
  let weightedCovered = 0;
  let weightedTotal = 0;
  let pctSum = 0;
  let pctCount = 0;

  for (const entry of matchedEntries) {
    const metricSummary = entry[metric];

    if (
      metricSummary
      && Number.isFinite(metricSummary.total)
      && Number.isFinite(metricSummary.covered)
      && metricSummary.total > 0
    ) {
      weightedCovered += metricSummary.covered;
      weightedTotal += metricSummary.total;
      continue;
    }

    const pct = readMetricPct(metricSummary);

    if (pct !== null) {
      pctSum += pct;
      pctCount += 1;
    }
  }

  if (weightedTotal > 0) {
    return (weightedCovered / weightedTotal) * 100;
  }

  if (pctCount > 0) {
    return pctSum / pctCount;
  }

  return 0;
}

function matchesFrontendThreshold(filePath, prefix) {
  return normalizeCoveragePath(filePath).includes(`/${prefix}`);
}

function collectFrontendCoverageFailures(summary) {
  const entries = Object.entries(summary).filter(([filePath]) => filePath !== 'total');

  return frontendThresholds.flatMap((threshold) => {
    const matchedEntries = entries
      .filter(([filePath]) => matchesFrontendThreshold(filePath, threshold.prefix))
      .map(([, coverage]) => coverage);

    return FRONTEND_METRICS.flatMap((metric) => {
      const actualPct = aggregateMetric(matchedEntries, metric);
      const expectedPct = threshold.thresholds[metric];

      if (Number(formatPct(actualPct)) >= Number(formatPct(expectedPct))) {
        return [];
      }

      return [{
        key: threshold.key,
        prefix: threshold.prefix,
        metric,
        expectedPct,
        actualPct,
      }];
    });
  });
}

function readBackendLinePct(summary, sourcePathMarker) {
  const files = summary?.data?.[0]?.files;
  if (sourcePathMarker && Array.isArray(files)) {
    const ownedLines = files
      .filter((file) => normalizeCoveragePath(file.filename || '').includes(sourcePathMarker))
      .map((file) => file.summary?.lines)
      .filter((lines) => Number.isFinite(lines?.count) && Number.isFinite(lines?.covered));
    if (ownedLines.length > 0) {
      const totals = ownedLines.reduce((accumulator, lines) => ({
        count: accumulator.count + lines.count,
        covered: accumulator.covered + lines.covered,
      }), { count: 0, covered: 0 });
      return totals.count === 0 ? 100 : (totals.covered / totals.count) * 100;
    }
  }
  return summary?.data?.[0]?.totals?.lines?.percent ?? 0;
}

function collectBackendCoverageFailures(summaries, backendKeys) {
  return selectBackendCoverageEntries(backendKeys).flatMap((threshold) => {
    const actualPct = readBackendLinePct(summaries[threshold.key], threshold.sourcePathMarker);
    const expectedPct = threshold.line;

    if (actualPct + Number.EPSILON >= expectedPct) {
      return [];
    }

    return [{
      key: threshold.key,
      metric: 'lines',
      expectedPct,
      actualPct,
    }];
  });
}

function readJsonFile(filePath, readFileSyncImpl = fs.readFileSync) {
  return JSON.parse(readFileSyncImpl(filePath, 'utf8'));
}

function loadFrontendCoverageSummary(repoRoot, readFileSyncImpl = fs.readFileSync) {
  const appSummary = readJsonFile(
    path.join(repoRoot, COVERAGE_ROOT, 'frontend', 'coverage-summary.json'),
    readFileSyncImpl
  );
  const pageRuntimeSummaryPath = path.join(
    repoRoot,
    COVERAGE_ROOT,
    'frontend',
    'page-runtime',
    'coverage-summary.json'
  );

  if (!fs.existsSync(pageRuntimeSummaryPath)) {
    return appSummary;
  }

  const pageRuntimeSummary = readJsonFile(pageRuntimeSummaryPath, readFileSyncImpl);

  return {
    ...appSummary,
    ...Object.fromEntries(
      Object.entries(pageRuntimeSummary).filter(([filePath]) => filePath !== 'total')
    ),
  };
}

function loadBackendCoverageSummaries(repoRoot, readFileSyncImpl = fs.readFileSync, backendKeys) {
  return Object.fromEntries(
    selectBackendCoverageEntries(backendKeys).map((entry) => [
      entry.key,
      readJsonFile(
        path.join(repoRoot, COVERAGE_ROOT, 'backend', `${entry.key}.json`),
        readFileSyncImpl
      ),
    ])
  );
}

function formatPct(value) {
  return value.toFixed(2);
}

function reportCoverageThresholds({
  repoRoot,
  target,
  readFileSyncImpl = fs.readFileSync,
  writeStdout = (text) => process.stdout.write(text),
  writeStderr = (text) => process.stderr.write(text),
  backendKeys,
}) {
  const failures = [];

  if (target === 'frontend' || target === 'all') {
    failures.push(...collectFrontendCoverageFailures(loadFrontendCoverageSummary(repoRoot, readFileSyncImpl)));
  }

  if (target === 'backend' || target === 'all') {
    failures.push(...collectBackendCoverageFailures(
      loadBackendCoverageSummaries(repoRoot, readFileSyncImpl, backendKeys),
      backendKeys
    ));
  }

  if (failures.length > 0) {
    writeStderr(`[${COVERAGE_SCOPE_LABEL}] Coverage threshold failures:\n`);

    for (const failure of failures) {
      writeStderr(
        `- ${failure.key} ${failure.metric}: expected >= ${formatPct(failure.expectedPct)}%, `
          + `received ${formatPct(failure.actualPct)}%\n`
      );
    }

    return 1;
  }

  writeStdout(`[${COVERAGE_SCOPE_LABEL}] Coverage thresholds passed for ${target}.\n`);
  return 0;
}

module.exports = { selectBackendCoverageEntries, collectFrontendCoverageFailures, collectBackendCoverageFailures, reportCoverageThresholds, BACKEND_COVERAGE_ENTRY_BY_KEY };
