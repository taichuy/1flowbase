#!/usr/bin/env node

const {
  parseCliArgs,
  runRollbackGate,
  usage,
} = require('./core.js');

async function main(argv = process.argv.slice(2), deps = {}) {
  const options = parseCliArgs(argv, deps.env || process.env);
  if (options.help) {
    usage(deps.writeStdout || ((text) => process.stdout.write(text)));
    return 0;
  }

  return runRollbackGate(options, deps);
}

if (require.main === module) {
  main().then((exitCode) => {
    process.exitCode = exitCode;
  }).catch((error) => {
    process.stderr.write(`[release-rollback-gate] ${error.message}\n`);
    process.exitCode = 1;
  });
}

module.exports = {
  main,
};
