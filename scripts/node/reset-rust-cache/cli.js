#!/usr/bin/env node

const { runRustCacheReset } = require('./core.js');

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(`用法：node scripts/node/reset-rust-cache/cli.js

停止 api-server，删除 api/target，然后依次预热 workspace dev、
api-server dev-up 和 workspace test 构建目标。
`);
}

async function main({ argv = process.argv.slice(2), writeStdout } = {}) {
  if (argv.length === 1 && (argv[0] === '-h' || argv[0] === '--help')) {
    usage(writeStdout);
    return 0;
  }
  if (argv.length > 0) {
    throw new Error(`未知参数：${argv.join(' ')}`);
  }

  return runRustCacheReset({ writeStdout });
}

if (require.main === module) {
  main().then((status) => {
    process.exitCode = status;
  }).catch((error) => {
    process.stderr.write(`[1flowbase-reset-rust-cache] ${error.message}\n`);
    process.exitCode = 1;
  });
}

module.exports = {
  main,
  usage,
};
