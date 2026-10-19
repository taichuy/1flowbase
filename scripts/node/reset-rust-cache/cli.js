#!/usr/bin/env node

const { runRustCacheReset } = require('./core.js');

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(`用法：node scripts/node/reset-rust-cache/cli.js [--warmup-only]

停止当前 worktree 的 api-server，删除 api/target，获取 Cargo.lock 中的依赖，
编译 dev-up 使用的 api-server binary，再准备测试配置下的普通库和 binary 测试目标，
最后预热 workspace 全部测试目标（不运行测试）。阶段串行，内部默认使用半数可用逻辑 CPU，
目标最少 2，不超过实际可用数量；不修改共享的开发并发配置。
--warmup-only：首次开发初始化或补齐缓存时使用，保留 api/target 和运行中的服务。
`);
}

async function main({ argv = process.argv.slice(2), writeStdout, runImpl = runRustCacheReset } = {}) {
  if (argv.length === 1 && (argv[0] === '-h' || argv[0] === '--help')) {
    usage(writeStdout);
    return 0;
  }
  const warmupOnly = argv.length === 1 && argv[0] === '--warmup-only';
  if (argv.length > 0 && !warmupOnly) {
    throw new Error(`未知参数：${argv.join(' ')}`);
  }

  return runImpl({ writeStdout, warmupOnly });
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
