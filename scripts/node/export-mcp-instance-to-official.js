#!/usr/bin/env node

require('./export-mcp-instance-to-official/cli.js').main().catch((error) => {
  process.stderr.write(`${error.stack || error.message}\n`);
  process.exitCode = 1;
});
