#!/usr/bin/env node

const { main } = require('./build-local-deploy-images/core.js');

if (require.main === module) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`[1flowbase-build-local-deploy-images] ${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = {
  main,
};
