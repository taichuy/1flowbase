#!/usr/bin/env node

const { main } = require('./core.js');

main(process.argv.slice(2)).catch((error) => {
  process.stderr.write(`[1flowbase-tailwind-boundary] ${error.message}\n`);
  process.exitCode = 1;
});
