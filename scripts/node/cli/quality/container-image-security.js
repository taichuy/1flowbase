#!/usr/bin/env node

const { writeContainerImageSecurityReports } = require('../../container-image-security/core.js');

function main(_argv = [], deps = {}) {
  const result = writeContainerImageSecurityReports({
    repoRoot: deps.repoRoot,
    outputRoot: deps.outputRoot,
  });

  const writeStdout = deps.writeStdout || ((text) => process.stdout.write(text));
  writeStdout(
    `[container-image-security] ${result.report.status}: `
      + `${result.report.componentCount} component(s), `
      + `HIGH ${result.report.highCount}, CRITICAL ${result.report.criticalCount}\n`
  );

  return result.report.exitCode;
}

if (require.main === module) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`[container-image-security] ${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = {
  main,
};
