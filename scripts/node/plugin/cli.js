#!/usr/bin/env node

const { main } = require('./core.js');
const { log } = require('./fs.js');
const { createPluginPackage } = require('./package.js');
const path = require('node:path');

function packageUsage() {
  process.stdout.write(`用法：node scripts/node/plugin/cli.js package <plugin-path> --out <output-dir> --runtime-binary <file> --target <triple> [signing options]

签名 runtime core 选项：
  --signing-key-pem-file <file>
  --signing-key-id <id>
  --issued-at <iso8601>
  --runtime-core-binary <file>
  --runtime-core-gpl-license-notice <file>
  --runtime-core-corresponding-source <https-url>
`);
}

function parsePackageArgs(argv) {
  if (!argv[1]) {
    throw new Error('package 需要提供 <plugin-path>');
  }

  const options = {
    pluginPath: path.resolve(argv[1]),
    outputDir: null,
    runtimeBinaryFile: null,
    targetTriple: null,
    signingKeyPemFile: null,
    signingKeyId: null,
    issuedAt: null,
    runtimeCoreBinaryFile: null,
    runtimeCoreGplLicenseNoticeFile: null,
    runtimeCoreCorrespondingSource: null,
  };
  const values = argv.slice(2);

  for (let index = 0; index < values.length; index += 1) {
    const arg = values[index];
    const value = values[index + 1];
    if (!value) {
      throw new Error(`${arg} 需要值`);
    }
    if (arg === '--out') options.outputDir = path.resolve(value);
    else if (arg === '--runtime-binary') options.runtimeBinaryFile = path.resolve(value);
    else if (arg === '--target') options.targetTriple = value;
    else if (arg === '--signing-key-pem-file') options.signingKeyPemFile = path.resolve(value);
    else if (arg === '--signing-key-id') options.signingKeyId = value;
    else if (arg === '--issued-at') options.issuedAt = value;
    else if (arg === '--runtime-core-binary') {
      options.runtimeCoreBinaryFile = path.resolve(value);
    } else if (arg === '--runtime-core-gpl-license-notice') {
      options.runtimeCoreGplLicenseNoticeFile = path.resolve(value);
    } else if (arg === '--runtime-core-corresponding-source') {
      options.runtimeCoreCorrespondingSource = value;
    } else {
      throw new Error(`未知参数：${arg}`);
    }
    index += 1;
  }

  if (!options.outputDir) throw new Error('package 需要提供 --out <output-dir>');
  if (!options.runtimeBinaryFile) {
    throw new Error('package 需要 --runtime-binary 指向已编译 provider 可执行文件');
  }
  if (!options.targetTriple) throw new Error('package 需要 --target 指定 rust target triple');
  if (Boolean(options.signingKeyPemFile) !== Boolean(options.signingKeyId)) {
    throw new Error('package 使用签名时需要同时提供 signing key PEM 与 signing key id');
  }
  const runtimeCoreInputs = [
    options.runtimeCoreBinaryFile,
    options.runtimeCoreGplLicenseNoticeFile,
    options.runtimeCoreCorrespondingSource,
  ];
  if (runtimeCoreInputs.some(Boolean) && !options.signingKeyPemFile) {
    throw new Error('runtime core receipt 需要同时提供签名 key PEM 与 key id');
  }
  if (runtimeCoreInputs.some(Boolean) && !runtimeCoreInputs.every(Boolean)) {
    throw new Error('runtime core receipt 需要 --runtime-core-binary、GPL notice 和 corresponding source');
  }

  return options;
}

async function run(argv) {
  if (argv[0] !== 'package') {
    return main(argv);
  }
  if (argv.includes('-h') || argv.includes('--help')) {
    packageUsage();
    return null;
  }

  const options = parsePackageArgs(argv);
  const result = createPluginPackage(
    options.pluginPath,
    options.outputDir,
    options
  );
  log(`Plugin package created at ${result.packageFile}`);
  return result;
}

run(process.argv.slice(2)).catch((error) => {
  process.stderr.write(`[1flowbase-plugin] ${error.message}\n`);
  process.exitCode = 1;
});

module.exports = {
  parsePackageArgs,
  packageUsage,
  run,
};
