const path = require('node:path');

const { log } = require('./fs.js');
const {
  DEFAULT_DEMO_HOST,
  DEFAULT_DEMO_PORT,
  createPluginDemoScaffold,
  createPluginScaffold,
  resolvePort,
  startDemoServer,
} = require('./init.js');
const { createPluginPackage } = require('./package.js');

function usage() {
  process.stdout.write(`用法：node scripts/node/plugin/cli.js <command> [options]

命令：
  init [plugin-path]
    生成 model provider runtime extension 基础源码结构；未提供路径时默认使用当前目录。

  demo init <plugin-path>
    在目标插件目录下生成 demo 页面与本地辅助脚本。

  demo dev <plugin-path> [--host <host>] [--port <port>]
    启动目标插件目录下 demo/ 的本地静态服务。

  package <plugin-path> --out <output-dir>
    生成只包含运行时资源与 bin/ 可执行文件的 .1flowbasepkg 安装产物，并返回 sha256 元数据。
    可选传入官方签名参数，将 _meta/official-release.json 与 .sig 一并写入包内。

选项：
  --runtime-binary <file>  package 时写入 bin/ 的已编译 provider 可执行文件
  --target <triple>        package 时指定 rust target triple，例如 x86_64-unknown-linux-musl
  --host <host>        demo dev 监听地址，默认 127.0.0.1
  --port <port>        demo dev 监听端口，默认 4310；传 0 表示自动分配
  --signing-key-pem-file <file>  package 时使用的 ed25519 PKCS8 私钥 PEM 文件
  --signing-key-id <id>          package 时写入官方签名 key id
  --issued-at <iso8601>          package 时写入官方签名签发时间，默认当前 UTC 时间
  -h, --help           查看帮助

示例：
  node scripts/node/plugin/cli.js init ../1flowbase-official-plugins/runtime-extensions/model-providers/openai_compatible
  node scripts/node/plugin/cli.js demo init ../1flowbase-official-plugins/runtime-extensions/model-providers/openai_compatible
  node scripts/node/plugin/cli.js demo dev ../1flowbase-official-plugins/runtime-extensions/model-providers/openai_compatible --port 4310
  node scripts/node/plugin/cli.js package ../1flowbase-official-plugins/runtime-extensions/model-providers/openai_compatible --out ./dist
  node scripts/node/plugin/cli.js package ../1flowbase-official-plugins/runtime-extensions/model-providers/openai_compatible --out ./dist --signing-key-pem-file ./official-plugin-signing-key.pem --signing-key-id official-key-2026-04
`);
}

function parseCliArgs(argv) {
  if (argv.includes('-h') || argv.includes('--help') || argv.length === 0) {
    return { command: 'help' };
  }

  const [first, second, third, ...rest] = argv;

  if (first === 'init') {
    if (rest.length > 0) {
      throw new Error(`未知参数：${rest[0]}`);
    }
    return {
      command: 'init',
      pluginPath: second ? path.resolve(second) : process.cwd(),
    };
  }

  if (first === 'demo' && second === 'init') {
    if (!third) {
      throw new Error('demo init 需要提供 <plugin-path>');
    }
    if (rest.length > 0) {
      throw new Error(`未知参数：${rest[0]}`);
    }
    return {
      command: 'demo-init',
      pluginPath: path.resolve(third),
    };
  }

  if (first === 'demo' && second === 'dev') {
    if (!third) {
      throw new Error('demo dev 需要提供 <plugin-path>');
    }

    const options = {
      command: 'demo-dev',
      pluginPath: path.resolve(third),
      host: DEFAULT_DEMO_HOST,
      port: DEFAULT_DEMO_PORT,
    };

    for (let index = 0; index < rest.length; index += 1) {
      const arg = rest[index];
      const next = rest[index + 1];
      if (arg === '--host') {
        if (!next) {
          throw new Error('--host 需要值');
        }
        options.host = next;
        index += 1;
        continue;
      }
      if (arg === '--port') {
        if (!next) {
          throw new Error('--port 需要值');
        }
        options.port = resolvePort(next);
        index += 1;
        continue;
      }
      throw new Error(`未知参数：${arg}`);
    }

    return options;
  }

  if (first === 'package') {
    if (!second) {
      throw new Error('package 需要提供 <plugin-path>');
    }

    const packageArgs = [third, ...rest].filter(Boolean);
    const options = {
      command: 'package',
      pluginPath: path.resolve(second),
      outputDir: null,
      runtimeBinaryFile: null,
      targetTriple: null,
      signingKeyPemFile: null,
      signingKeyId: null,
      issuedAt: null,
    };

    for (let index = 0; index < packageArgs.length; index += 1) {
      const arg = packageArgs[index];
      const next = packageArgs[index + 1];
      if (arg === '--out') {
        if (!next) {
          throw new Error('--out 需要值');
        }
        options.outputDir = path.resolve(next);
        index += 1;
        continue;
      }
      if (arg === '--runtime-binary') {
        if (!next) {
          throw new Error('--runtime-binary 需要值');
        }
        options.runtimeBinaryFile = path.resolve(next);
        index += 1;
        continue;
      }
      if (arg === '--target') {
        if (!next) {
          throw new Error('--target 需要值');
        }
        options.targetTriple = next;
        index += 1;
        continue;
      }
      if (arg === '--signing-key-pem-file') {
        if (!next) {
          throw new Error('--signing-key-pem-file 需要值');
        }
        options.signingKeyPemFile = path.resolve(next);
        index += 1;
        continue;
      }
      if (arg === '--signing-key-id') {
        if (!next) {
          throw new Error('--signing-key-id 需要值');
        }
        options.signingKeyId = next;
        index += 1;
        continue;
      }
      if (arg === '--issued-at') {
        if (!next) {
          throw new Error('--issued-at 需要值');
        }
        options.issuedAt = next;
        index += 1;
        continue;
      }
      throw new Error(`未知参数：${arg}`);
    }

    if (!options.outputDir) {
      throw new Error('package 需要提供 --out <output-dir>');
    }
    if (!options.runtimeBinaryFile) {
      throw new Error('package 需要 --runtime-binary 指向已编译 provider 可执行文件');
    }
    if (!options.targetTriple) {
      throw new Error('package 需要 --target 指定 rust target triple');
    }
    if (options.signingKeyPemFile && !options.signingKeyId) {
      throw new Error('package 使用签名时需要提供 --signing-key-id');
    }
    if (options.signingKeyId && !options.signingKeyPemFile) {
      throw new Error('package 使用签名时需要提供 --signing-key-pem-file');
    }

    return options;
  }

  throw new Error(`未知命令：${argv.join(' ')}`);
}

async function waitForTermination(handle) {
  await new Promise((resolve) => {
    const shutdown = async () => {
      process.off('SIGINT', shutdown);
      process.off('SIGTERM', shutdown);
      await handle.close();
      resolve();
    };

    process.on('SIGINT', shutdown);
    process.on('SIGTERM', shutdown);
  });
}

async function main(argv) {
  const parsed = parseCliArgs(argv);

  if (parsed.command === 'help') {
    usage();
    return null;
  }

  if (parsed.command === 'init') {
    const result = createPluginScaffold(parsed.pluginPath);
    log(`Plugin scaffold created at ${result.pluginPath}`);
    return result;
  }

  if (parsed.command === 'demo-init') {
    const result = createPluginDemoScaffold(parsed.pluginPath);
    log(`Demo scaffold created at ${path.join(result.pluginPath, 'demo')}`);
    return result;
  }

  if (parsed.command === 'demo-dev') {
    const handle = await startDemoServer(parsed);
    await waitForTermination(handle);
    return handle;
  }

  if (parsed.command === 'package') {
    const result = createPluginPackage(parsed.pluginPath, parsed.outputDir, parsed);
    log(`Plugin package created at ${result.packageFile}`);
    return result;
  }

  throw new Error(`未知命令：${parsed.command}`);
}

module.exports = {
  DEFAULT_DEMO_HOST,
  DEFAULT_DEMO_PORT,
  createPluginDemoScaffold,
  createPluginPackage,
  createPluginScaffold,
  main,
  parseCliArgs,
  startDemoServer,
};
