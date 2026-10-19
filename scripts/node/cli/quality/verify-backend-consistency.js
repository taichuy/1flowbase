#!/usr/bin/env node

const {
  buildBackendConsistencyCommands: buildCommands,
  runBackendConsistency: main,
  runBackendConsistencyCommandSequence,
} = require('../../verify/index.js');

if (require.main === module) {
  Promise.resolve()
    .then(() => main(process.argv.slice(2)))
    .then((status) => {
      process.exitCode = status;
    })
    .catch((error) => {
      process.stderr.write(`[1flowbase-verify-backend-consistency] ${error.message}\n`);
      process.exitCode = 1;
    });
}

module.exports = {
  buildCommands,
  main,
  runBackendConsistencyCommandSequence,
};
