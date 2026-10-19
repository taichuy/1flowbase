#!/usr/bin/env node

const {
  CONTRACT_TEST_FILES,
  buildContractsCommands: buildCommands,
  runContracts: main,
} = require('../../test/index.js');

if (require.main === module) {
  Promise.resolve()
    .then(() => main(process.argv.slice(2)))
    .then((status) => {
      process.exitCode = status;
    })
    .catch((error) => {
      process.stderr.write(`[1flowbase-test-contracts] ${error.message}\n`);
      process.exitCode = 1;
    });
}

module.exports = {
  CONTRACT_TEST_FILES,
  buildCommands,
  main,
};
