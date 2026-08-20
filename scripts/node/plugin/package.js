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
const {
  payloadSha256,
  writeOfficialSignatureFiles,
  writeRuntimeCoreComplianceMetadata,
} = require('./release.js');

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

function readRuntimeEntry(manifestPath) {
  const lines = fs.readFileSync(manifestPath, 'utf8').split(/\r?\n/);
  let runtimeIndent = null;

  for (const line of lines) {
    const runtimeMatch = line.match(/^(\s*)runtime:\s*(?:#.*)?$/);
    if (runtimeMatch) {
      runtimeIndent = runtimeMatch[1].length;
      continue;
    }

    if (runtimeIndent === null || /^\s*(?:#.*)?$/.test(line)) {
      continue;
    }

    const indent = line.match(/^\s*/)[0].length;
    if (indent <= runtimeIndent) {
      runtimeIndent = null;
      continue;
    }

    const entryMatch = line.match(/^\s*entry:\s*([^\s#]+)\s*(?:#.*)?$/);
    if (entryMatch) {
      return entryMatch[1];
    }
  }

  throw new Error('manifest 必须声明 runtime.entry');
}

function assertSupportedRuntimeEntry(entry) {
  if (
    !entry ||
    entry.includes('\\') ||
    path.posix.isAbsolute(entry) ||
    path.win32.isAbsolute(entry)
  ) {
    throw new Error('runtime.entry 必须是安全的 bin/ 相对可执行文件路径');
  }

  const segments = entry.split('/');
  if (
    segments.length !== 2 ||
    segments[0] !== 'bin' ||
    !/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(segments[1])
  ) {
    throw new Error('runtime.entry 必须是安全的 bin/ 相对可执行文件路径');
  }

  return entry;
}

function assertSupportedPackageIdentity(pluginCode) {
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(pluginCode)) {
    throw new Error('plugin_id 必须是打包器支持的稳定身份');
  }

  return pluginCode;
}

function runtimeEntryForTarget(runtimeEntry, target) {
  if (
    target.executableSuffix &&
    !runtimeEntry.toLowerCase().endsWith(target.executableSuffix)
  ) {
    return `${runtimeEntry}${target.executableSuffix}`;
  }

  return runtimeEntry;
}

function runtimeCoreArtifactForTarget(target) {
  return [
    'runtime-core',
    target.rustTargetTriple,
    `1flowbase-runtime-core${target.executableSuffix}`,
  ].join('/');
}

function writeStagedRuntimeEntry(manifestPath, runtimeEntry) {
  const lines = fs.readFileSync(manifestPath, 'utf8').split(/\r?\n/);
  let runtimeIndent = null;

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const runtimeMatch = line.match(/^(\s*)runtime:\s*(?:#.*)?$/);
    if (runtimeMatch) {
      runtimeIndent = runtimeMatch[1].length;
      continue;
    }

    if (runtimeIndent === null || /^\s*(?:#.*)?$/.test(line)) {
      continue;
    }

    const indent = line.match(/^\s*/)[0].length;
    if (indent <= runtimeIndent) {
      runtimeIndent = null;
      continue;
    }

    if (/^\s*entry:\s*[^\s#]+\s*(?:#.*)?$/.test(line)) {
      lines[index] = `${line.match(/^\s*/)[0]}entry: ${runtimeEntry}`;
      fs.writeFileSync(manifestPath, lines.join('\n'), 'utf8');
      return;
    }
  }

  throw new Error('staged manifest 必须声明 runtime.entry');
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
  const runtimeCoreBinaryFile = options.runtimeCoreBinaryFile
    ? path.resolve(options.runtimeCoreBinaryFile)
    : null;
  if (!runtimeBinaryFile) {
    throw new Error('package 需要 --runtime-binary 指向已编译 provider 可执行文件');
  }
  if (!options.targetTriple) {
    throw new Error('package 需要 --target 指定 rust target triple');
  }
  const hasSigningKeyPemFile = Boolean(options.signingKeyPemFile);
  const hasSigningKeyId = Boolean(options.signingKeyId);
  if (hasSigningKeyPemFile !== hasSigningKeyId) {
    throw new Error('package 使用签名时需要同时提供 signing key PEM 与 signing key id');
  }
  const target = parseRustTargetTriple(options.targetTriple);
  const stagedRoot = createPackageArtifactRoot(resolvedPluginPath);
  const pluginCode = assertSupportedPackageIdentity(readPluginCode(resolvedPluginPath));
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
  const runtimeEntry = assertSupportedRuntimeEntry(
    readRuntimeEntry(path.join(resolvedPluginPath, 'manifest.yaml'))
  );
  const stagedRuntimeEntry = runtimeEntryForTarget(runtimeEntry, target);

  if (!fs.existsSync(runtimeBinaryFile)) {
    throw new Error(`runtime binary 不存在：${runtimeBinaryFile}`);
  }

  fs.mkdirSync(resolvedOutputDir, { recursive: true });

  if (stagedRuntimeEntry !== runtimeEntry) {
    writeStagedRuntimeEntry(
      path.join(stagedRoot, 'manifest.yaml'),
      stagedRuntimeEntry
    );
  }

  const stagedBinaryPath = path.join(stagedRoot, ...stagedRuntimeEntry.split('/'));
  fs.mkdirSync(path.dirname(stagedBinaryPath), { recursive: true });
  fs.copyFileSync(runtimeBinaryFile, stagedBinaryPath);
  fs.chmodSync(stagedBinaryPath, 0o755);

  const pendingFile = path.join(
    resolvedOutputDir,
    `${vendor}@${pluginCode}@${version}@${target.assetSuffix}@pending.1flowbasepkg`
  );

  try {
    let signatureMetadata = null;
    let runtimeCore = null;
    if (hasSigningKeyPemFile) {
      if (!runtimeCoreBinaryFile) {
        throw new Error('runtime core 签名包需要 --runtime-core-binary 指向已编译 runtime core CLI');
      }
      if (runtimeCoreBinaryFile === runtimeBinaryFile) {
        throw new Error('runtime core binary 必须与 manifest.runtime.entry wrapper 使用不同输入文件');
      }
      if (!options.runtimeCoreGplLicenseNoticeFile) {
        throw new Error('runtime core 签名包需要 GPL license notice');
      }
      if (!options.runtimeCoreCorrespondingSource) {
        throw new Error('runtime core 签名包需要 corresponding source pointer');
      }
      if (!fs.existsSync(runtimeCoreBinaryFile)) {
        throw new Error(`runtime core binary 不存在：${runtimeCoreBinaryFile}`);
      }
      if (!fs.statSync(runtimeCoreBinaryFile).isFile()) {
        throw new Error(`runtime core binary 必须是普通文件：${runtimeCoreBinaryFile}`);
      }
      const runtimeCoreArtifactPath = runtimeCoreArtifactForTarget(target);
      const stagedRuntimeCorePath = path.join(
        stagedRoot,
        ...runtimeCoreArtifactPath.split('/')
      );
      fs.mkdirSync(path.dirname(stagedRuntimeCorePath), { recursive: true });
      fs.copyFileSync(runtimeCoreBinaryFile, stagedRuntimeCorePath);
      fs.chmodSync(stagedRuntimeCorePath, 0o755);
      runtimeCore = writeRuntimeCoreComplianceMetadata(stagedRoot, {
        artifactPath: runtimeCoreArtifactPath,
        targetTriple: target.rustTargetTriple,
        gplLicenseNoticeFile: path.resolve(options.runtimeCoreGplLicenseNoticeFile),
        correspondingSource: options.runtimeCoreCorrespondingSource,
      });
      signatureMetadata = writeOfficialSignatureFiles(stagedRoot, {
        pluginId: manifestPluginId,
        providerCode: pluginCode,
        version,
        contractVersion,
        artifactSha256: payloadSha256(stagedRoot),
        signingKeyPemFile: path.resolve(options.signingKeyPemFile),
        signingKeyId: options.signingKeyId,
        issuedAt: options.issuedAt || new Date().toISOString(),
        runtimeCore,
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
      runtimeCoreArtifactSha256: runtimeCore?.artifact_sha256 ?? null,
      runtimeCoreCorrespondingSource: runtimeCore?.corresponding_source ?? null,
    };
  } finally {
    removeDirIfExists(stagedRoot);
    removeDirIfExists(pendingFile);
  }
}

module.exports = {
  assertSupportedRuntimeEntry,
  createPluginPackage,
  parseRustTargetTriple,
  runtimeCoreArtifactForTarget,
  runtimeEntryForTarget,
};
