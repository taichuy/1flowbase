#!/usr/bin/env node

const { runRustCacheReset } = require('./core.js');

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(`用法：node scripts/node/reset-rust-cache/cli.js

停止当前 worktree 的 api-server，删除 api/target，获取 Cargo.lock 中的依赖，
编译 dev-up 使用的 api-server binary，再准备测试配置下的普通库和 binary 测试目标，
最后预热 workspace 全部测试目标（不运行测试）。阶段串行，内部使用可用逻辑 CPU 的一半
作为 Cargo 并发预算（向下取整，最少 1）；不修改共享的开发并发配置。
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
