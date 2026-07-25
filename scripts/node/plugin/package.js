const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const {
  createPackageArtifactRoot,
  ensurePluginScaffoldExists,
  removeDirIfExists,
} = require('./fs.js');
const { readManifestField, readPluginCode } = require('./manifest.js');
const { payloadSha256, writeOfficialSignatureFiles } = require('./release.js');

function hashFile(filePath) {
  return crypto.createHash('sha256').update(fs.readFileSync(filePath)).digest('hex');
}

function createTarArchive(archivePath, sourceDir) {
  const archiveFd = fs.openSync(archivePath, 'w');

  try {
    const result = spawnSync('tar', ['-czf', '-', '.'], {
      cwd: sourceDir,
      stdio: ['ignore', archiveFd, 'pipe'],
    });

    if (result.error) {
      throw result.error;
    }
    if (result.status !== 0) {
      const stderr = result.stderr ? result.stderr.toString('utf8').trim() : '';
      throw new Error(stderr || `tar 打包失败，退出码 ${result.status}`);
    }
  } finally {
    fs.closeSync(archiveFd);
  }
}

function parseRustTargetTriple(raw) {
  switch (String(raw || '').trim()) {
    case 'x86_64-unknown-linux-gnu':
      return {
        rustTargetTriple: raw,
        os: 'linux',
        arch: 'amd64',
        libc: 'gnu',
        assetSuffix: 'linux-amd64',
        executableSuffix: '',
      };
    case 'aarch64-unknown-linux-gnu':
      return {
        rustTargetTriple: raw,
        os: 'linux',
        arch: 'arm64',
        libc: 'gnu',
        assetSuffix: 'linux-arm64',
        executableSuffix: '',
      };
    case 'x86_64-unknown-linux-musl':
      return {
        rustTargetTriple: raw,
        os: 'linux',
        arch: 'amd64',
        libc: 'musl',
        assetSuffix: 'linux-amd64',
        executableSuffix: '',
      };
    case 'aarch64-unknown-linux-musl':
      return {
        rustTargetTriple: raw,
        os: 'linux',
        arch: 'arm64',
        libc: 'musl',
        assetSuffix: 'linux-arm64',
        executableSuffix: '',
      };
    case 'x86_64-apple-darwin':
      return {
        rustTargetTriple: raw,
        os: 'darwin',
        arch: 'amd64',
        libc: null,
        assetSuffix: 'darwin-amd64',
        executableSuffix: '',
      };
    case 'aarch64-apple-darwin':
      return {
        rustTargetTriple: raw,
        os: 'darwin',
        arch: 'arm64',
        libc: null,
        assetSuffix: 'darwin-arm64',
        executableSuffix: '',
      };
    case 'x86_64-pc-windows-msvc':
      return {
        rustTargetTriple: raw,
        os: 'windows',
        arch: 'amd64',
        libc: 'msvc',
        assetSuffix: 'windows-amd64',
        executableSuffix: '.exe',
      };
    case 'aarch64-pc-windows-msvc':
      return {
        rustTargetTriple: raw,
        os: 'windows',
        arch: 'arm64',
        libc: 'msvc',
        assetSuffix: 'windows-arm64',
        executableSuffix: '.exe',
      };
    default:
      throw new Error(`暂不支持的 rust target: ${raw}`);
  }
}

function createPluginPackage(pluginPath, outputDir, options = {}) {
  ensurePluginScaffoldExists(pluginPath);

  const resolvedPluginPath = path.resolve(pluginPath);
  const resolvedOutputDir = path.resolve(outputDir);
  const runtimeBinaryFile = options.runtimeBinaryFile
    ? path.resolve(options.runtimeBinaryFile)
    : null;
  if (!runtimeBinaryFile) {
    throw new Error('package 需要 --runtime-binary 指向已编译 provider 可执行文件');
  }
  if (!options.targetTriple) {
    throw new Error('package 需要 --target 指定 rust target triple');
  }
  const target = parseRustTargetTriple(options.targetTriple);
  const stagedRoot = createPackageArtifactRoot(resolvedPluginPath);
  const pluginCode = readPluginCode(resolvedPluginPath);
  const version = readManifestField(resolvedPluginPath, 'version', '0.1.0');
  const manifestPluginId = readManifestField(
    resolvedPluginPath,
    'plugin_id',
    pluginCode
  );
  const vendor = readManifestField(resolvedPluginPath, 'vendor', '1flowbase');
  const contractVersion = readManifestField(
    resolvedPluginPath,
    'contract_version',
    '1flowbase.provider/v2'
  );

  if (!fs.existsSync(runtimeBinaryFile)) {
    throw new Error(`runtime binary 不存在：${runtimeBinaryFile}`);
  }

  fs.mkdirSync(resolvedOutputDir, { recursive: true });

  const binaryName = `${pluginCode}-provider${target.executableSuffix}`;
  const stagedBinaryPath = path.join(stagedRoot, 'bin', binaryName);
  fs.mkdirSync(path.dirname(stagedBinaryPath), { recursive: true });
  fs.copyFileSync(runtimeBinaryFile, stagedBinaryPath);
  fs.chmodSync(stagedBinaryPath, 0o755);

  const pendingFile = path.join(
    resolvedOutputDir,
    `${vendor}@${pluginCode}@${version}@${target.assetSuffix}@pending.1flowbasepkg`
  );

  try {
    let signatureMetadata = null;
    if (options.signingKeyPemFile && options.signingKeyId) {
      signatureMetadata = writeOfficialSignatureFiles(stagedRoot, {
        pluginId: manifestPluginId,
        providerCode: pluginCode,
        version,
        contractVersion,
        artifactSha256: payloadSha256(stagedRoot),
        signingKeyPemFile: path.resolve(options.signingKeyPemFile),
        signingKeyId: options.signingKeyId,
        issuedAt: options.issuedAt || new Date().toISOString(),
      });
    }

    createTarArchive(pendingFile, stagedRoot);

    const checksum = hashFile(pendingFile);
    const finalFile = path.join(
      resolvedOutputDir,
      `${vendor}@${pluginCode}@${version}@${target.assetSuffix}@${checksum}.1flowbasepkg`
    );
    fs.renameSync(pendingFile, finalFile);

    return {
      pluginPath: resolvedPluginPath,
      packageFile: finalFile,
      packageName: path.basename(finalFile),
      checksum,
      os: target.os,
      arch: target.arch,
      libc: target.libc,
      rustTarget: target.rustTargetTriple,
      signatureAlgorithm: signatureMetadata?.signatureAlgorithm ?? null,
      signingKeyId: signatureMetadata?.signingKeyId ?? null,
    };
  } finally {
    removeDirIfExists(stagedRoot);
    removeDirIfExists(pendingFile);
  }
}

module.exports = {
  createPluginPackage,
  parseRustTargetTriple,
};
