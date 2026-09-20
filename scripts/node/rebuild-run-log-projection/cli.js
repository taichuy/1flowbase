#!/usr/bin/env node

const { main, parseCliArgs } = require('./core.js');

if (require.main === module) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`[1flowbase-rebuild-run-log-projection] ${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = {
  main,
  parseCliArgs
};
