const { stripVTControlCharacters } = require('node:util');

function parseCargoTestCounts(output) {
  const counts = {
    passedCount: null,
    failedCount: null,
  };
  const pattern = /test result:\s+(?:ok|FAILED)\.\s+(\d+) passed;\s+(\d+) failed;/gu;
  let match = pattern.exec(stripVTControlCharacters(output));

  while (match) {
    counts.passedCount = (counts.passedCount ?? 0) + Number.parseInt(match[1], 10);
    counts.failedCount = (counts.failedCount ?? 0) + Number.parseInt(match[2], 10);
    match = pattern.exec(stripVTControlCharacters(output));
  }

  return counts;
}

function requireExecutedCargoTests(output) {
  const counts = parseCargoTestCounts(output);
  if (!(counts.passedCount > 0 && counts.failedCount === 0)) {
    throw new Error('no executed passing tests or failed tests; refusing invalid Cargo test evidence');
  }
  return counts;
}

// Validate each command log independently; a neighboring passing suite cannot
// supply evidence for an empty selector, ignored suite, or compile-only command.
if (require.main === module) {
  const fs = require('node:fs');
  try {
    const logs = process.argv.slice(2);
    if (logs.length === 0) throw new Error('at least one Cargo test log is required');
    for (const log of logs) {
      const counts = requireExecutedCargoTests(fs.readFileSync(log, 'utf8'));
      process.stdout.write(`${log}: ${counts.passedCount} passed, ${counts.failedCount} failed\n`);
    }
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = { parseCargoTestCounts, requireExecutedCargoTests };
