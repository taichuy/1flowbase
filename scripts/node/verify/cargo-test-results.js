function parseCargoTestCounts(output) {
  const counts = {
    passedCount: null,
    failedCount: null,
  };
  const pattern = /test result:\s+(?:ok|FAILED)\.\s+(\d+) passed;\s+(\d+) failed;/gu;
  let match = pattern.exec(output);

  while (match) {
    counts.passedCount = (counts.passedCount ?? 0) + Number.parseInt(match[1], 10);
    counts.failedCount = (counts.failedCount ?? 0) + Number.parseInt(match[2], 10);
    match = pattern.exec(output);
  }

  return counts;
}

module.exports = { parseCargoTestCounts };
