#!/usr/bin/env node

const { main } = require('./reset-account-password/cli.js');

main().then(
  (exitCode) => {
    process.exitCode = exitCode;
  },
  (error) => {
    process.stderr.write(`[1flowbase-reset-account-password] ${error.message}\n`);
    process.exitCode = 1;
  }
);
