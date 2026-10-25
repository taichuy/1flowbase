const { stripVTControlCharacters } = require('node:util');
const path = require('node:path');

function parseNormalizedCargoTestCounts(normalizedOutput) {
  const counts = {
    passedCount: null,
    failedCount: null,
  };
  const pattern = /test result:\s+(?:ok|FAILED)\.\s+(\d+) passed;\s+(\d+) failed;/gu;
  let match = pattern.exec(normalizedOutput);

  while (match) {
    counts.passedCount = (counts.passedCount ?? 0) + Number.parseInt(match[1], 10);
    counts.failedCount = (counts.failedCount ?? 0) + Number.parseInt(match[2], 10);
    match = pattern.exec(normalizedOutput);
  }

  return counts;
}

function parseCargoTestCounts(output) {
  return parseNormalizedCargoTestCounts(stripVTControlCharacters(output));
}

function requireExecutedCargoTests(output, requiredFilters = []) {
  if (!Array.isArray(requiredFilters)
    || requiredFilters.some((filter) => typeof filter !== 'string' || !filter.trim())) {
    throw new Error('Cargo test filters must be non-empty strings');
  }
  const normalizedOutput = stripVTControlCharacters(output);
  const counts = parseNormalizedCargoTestCounts(normalizedOutput);
  if (!(counts.passedCount > 0 && counts.failedCount === 0)) {
    throw new Error('no executed passing tests or failed tests; refusing invalid Cargo test evidence');
  }
  if (requiredFilters.length > 0) {
    const passedNames = [...normalizedOutput.matchAll(/^test (.+) \.\.\. ok\r?$/gmu)]
      .map((match) => match[1]);
    for (const filter of requiredFilters) {
      if (!passedNames.some((name) => name.includes(filter))) {
        throw new Error(`${filter}: no executed passing tests; refusing empty Cargo test selection`);
      }
    }
  }
  return counts;
}

// Cargo runs libtest binaries serially. Keep their stderr launch order paired
// with stdout suites so a neighboring package cannot satisfy a protected filter.
function requireCargoTestTargets({ stdout = '', stderr = '' }, targets) {
  const launches = [...stripVTControlCharacters(stderr).matchAll(
    /^\s*(?:Running .+ \(([^\r\n]+)\)|Doc-tests\s+(\S+))\s*$/gmu,
  )];
  const suites = [...stripVTControlCharacters(stdout).matchAll(
    /^running \d+ tests?\r?\n([\s\S]*?)^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed;[^\r\n]*$/gmu,
  )];
  if (launches.length === 0 || launches.length !== suites.length) {
    throw new Error('Cargo launch/suite evidence is incomplete; refusing protected target reuse');
  }
  const executions = suites.map((suite, index) => ({
    binary: launches[index][1]
      ? path.basename(launches[index][1]).replace(/-[\da-f]+(?:\.exe)?$/u, '')
      : null,
    passed: [...suite[1].matchAll(/^test (.+) \.\.\. ok\r?$/gmu)].map((match) => match[1]),
  }));
  for (const target of targets) {
    const binary = target.testTarget || target.packageName.replaceAll('-', '_');
    if (!executions.some((suite) => suite.binary === binary
      && suite.passed.some((name) => name.includes(target.filter || '')))) {
      throw new Error(`${target.label}: no executed passing tests in ${binary}; refusing protected target reuse`);
    }
  }
}

// Usage: cargo-test-results.js <log> [<log> ...] [-- <filter> ...]
// Validate each log and optional filter independently; another log or filter
// cannot supply evidence for an empty selector, ignored suite, or compilation.
if (require.main === module) {
  const fs = require('node:fs');
  try {
    const args = process.argv.slice(2);
    const separator = args.indexOf('--');
    const logs = separator === -1 ? args : args.slice(0, separator);
    const requiredFilters = separator === -1 ? [] : args.slice(separator + 1);
    if (logs.length === 0) throw new Error('at least one Cargo test log is required');
    if (separator !== -1 && requiredFilters.length === 0) {
      throw new Error('at least one Cargo test filter is required after --');
    }
    for (const log of logs) {
      const counts = requireExecutedCargoTests(fs.readFileSync(log, 'utf8'), requiredFilters);
      process.stdout.write(`${log}: ${counts.passedCount} passed, ${counts.failedCount} failed\n`);
    }
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = { parseCargoTestCounts, requireExecutedCargoTests, requireCargoTestTargets };
