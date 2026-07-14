#!/usr/bin/env node

const { main } = require('./core.js');

if (require.main === module) {
  Promise.resolve()
    .then(() => main(process.argv.slice(2)))
    .then((status) => {
      process.exitCode = status;
    })
    .catch((error) => {
      process.stderr.write(`[1flowbase-console-operation-registry-hygiene] ${error.message}\n`);
      process.exitCode = 1;
    });
}

module.exports = {
  main,
};
