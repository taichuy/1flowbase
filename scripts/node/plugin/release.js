const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const { compareStablePath } = require('./fs.js');

const RUNTIME_CORE_LICENSE_NOTICE_PATH =
  '_meta/runtime-core-gpl-license-notice.txt';

function sha256Buffer(content) {
  return `sha256:${crypto.createHash('sha256').update(content).digest('hex')}`;
}

function sha256File(filePath) {
  return sha256Buffer(fs.readFileSync(filePath));
}

function assertRegularFile(filePath, label) {
  if (!filePath || !fs.existsSync(filePath) || !fs.statSync(filePath).isFile()) {
    throw new Error(`${label} 必须是存在的普通文件`);
  }
}

function assertGplLicenseNotice(notice) {
  if (!notice.trim()) {
    throw new Error('runtime core GPL license notice 不能为空');
  }
  if (!/GNU\s+GENERAL\s+PUBLIC\s+LICENSE|\bGPL(?:[-\s]*v?(?:2|3)(?:\.0)?(?:[-\s]*(?:or[-\s]*later|only))?)?\b/iu.test(notice)) {
    throw new Error('runtime core GPL license notice 必须明确声明 GNU GPL');
  }
}

function assertCorrespondingSource(pointer) {
  if (!pointer || !String(pointer).trim()) {
    throw new Error('runtime core 签名包需要 corresponding source pointer');
  }

  let url;
  try {
    url = new URL(String(pointer));
  } catch {
    throw new Error('runtime core corresponding source pointer 必须是有效的 HTTPS URL');
  }

  if (
    url.protocol !== 'https:' ||
    !url.hostname ||
    url.username ||
    url.password
  ) {
    throw new Error('runtime core corresponding source pointer 必须是有效的 HTTPS URL');
  }

  return url.toString();
}

function assertRuntimeCoreArtifactPath(artifactPath) {
  if (
    !artifactPath ||
    path.posix.isAbsolute(artifactPath) ||
    artifactPath.includes('\\')
  ) {
    throw new Error('runtime core artifact_path 必须是安全的 archive 相对路径');
  }

  const segments = artifactPath.split('/');
  if (
    segments.length !== 3 ||
    segments[0] !== 'runtime-core' ||
    !/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(segments[1]) ||
    !/^1flowbase-runtime-core(?:\.exe)?$/.test(segments[2])
  ) {
    throw new Error('runtime core artifact_path 必须位于安全的 runtime-core/<target>/ 路径');
  }

  return artifactPath;
}

function writeRuntimeCoreComplianceMetadata(stagedRoot, options) {
  const artifactPath = assertRuntimeCoreArtifactPath(options.artifactPath);
  const stagedArtifactPath = path.join(stagedRoot, ...artifactPath.split('/'));
  assertRegularFile(stagedArtifactPath, 'runtime core artifact');
  assertRegularFile(options.gplLicenseNoticeFile, 'runtime core GPL license notice');

  const licenseNotice = fs.readFileSync(options.gplLicenseNoticeFile, 'utf8');
  assertGplLicenseNotice(licenseNotice);
  const correspondingSource = assertCorrespondingSource(
    options.correspondingSource
  );
  const noticePath = path.join(stagedRoot, ...RUNTIME_CORE_LICENSE_NOTICE_PATH.split('/'));

  fs.mkdirSync(path.dirname(noticePath), { recursive: true });
  fs.writeFileSync(noticePath, licenseNotice, 'utf8');

  return {
    target_triple: String(options.targetTriple),
    artifact_path: artifactPath,
    artifact_sha256: sha256File(stagedArtifactPath),
    gpl_license_notice_path: RUNTIME_CORE_LICENSE_NOTICE_PATH,
    gpl_license_notice_sha256: sha256File(noticePath),
    corresponding_source: correspondingSource,
  };
}

function payloadSha256(rootDir) {
  const files = [];

  function walk(currentDir) {
    const children = fs
      .readdirSync(currentDir, { withFileTypes: true })
      .sort((left, right) => compareStablePath(left.name, right.name));

    for (const child of children) {
      const absolutePath = path.join(currentDir, child.name);
      const relativePath = path
        .relative(rootDir, absolutePath)
        .split(path.sep)
        .join('/');

      if (relativePath.startsWith('_meta/')) {
        continue;
      }

      if (child.isDirectory()) {
        walk(absolutePath);
        continue;
      }

      files.push([relativePath, fs.readFileSync(absolutePath)]);
    }
  }

  walk(rootDir);
  files.sort((left, right) => compareStablePath(left[0], right[0]));

  const hasher = crypto.createHash('sha256');
  for (const [relativePath, content] of files) {
    hasher.update(relativePath);
    hasher.update(Buffer.from([0]));
    hasher.update(content);
    hasher.update(Buffer.from([0]));
  }

  return `sha256:${hasher.digest('hex')}`;
}

function writeOfficialSignatureFiles(stagedRoot, options) {
  if (!options.runtimeCore || typeof options.runtimeCore !== 'object') {
    throw new Error('official release 签名需要 runtime_core receipt');
  }
  const privateKeyPem = fs.readFileSync(options.signingKeyPemFile, 'utf8');
  const privateKey = crypto.createPrivateKey(privateKeyPem);
  const release = {
    schema_version: 1,
    plugin_id: options.pluginId,
    provider_code: options.providerCode,
    version: options.version,
    contract_version: options.contractVersion,
    artifact_sha256: options.artifactSha256,
    payload_sha256: payloadSha256(stagedRoot),
    signature_algorithm: 'ed25519',
    signing_key_id: options.signingKeyId,
    issued_at: options.issuedAt,
    runtime_core: options.runtimeCore,
  };
  const releaseBytes = Buffer.from(JSON.stringify(release), 'utf8');
  const signature = crypto.sign(null, releaseBytes, privateKey);
  const metaDir = path.join(stagedRoot, '_meta');

  fs.mkdirSync(metaDir, { recursive: true });
  fs.writeFileSync(path.join(metaDir, 'official-release.json'), releaseBytes);
  fs.writeFileSync(path.join(metaDir, 'official-release.sig'), signature);

  return {
    signatureAlgorithm: release.signature_algorithm,
    signingKeyId: release.signing_key_id,
  };
}

function verifySignedRuntimeCoreRelease(stagedRoot, trustedPublicKey) {
  const releasePath = path.join(stagedRoot, '_meta', 'official-release.json');
  const signaturePath = path.join(stagedRoot, '_meta', 'official-release.sig');
  assertRegularFile(releasePath, 'official release receipt');
  assertRegularFile(signaturePath, 'official release signature');

  const releaseBytes = fs.readFileSync(releasePath);
  let release;
  try {
    release = JSON.parse(releaseBytes.toString('utf8'));
  } catch {
    throw new Error('official release receipt 必须是有效 JSON');
  }

  if (
    !crypto.verify(
      null,
      releaseBytes,
      trustedPublicKey,
      fs.readFileSync(signaturePath)
    )
  ) {
    throw new Error('official release signature 无效或已被篡改');
  }

  const runtimeCore = release.runtime_core;
  if (!runtimeCore || typeof runtimeCore !== 'object' || Array.isArray(runtimeCore)) {
    throw new Error('official release receipt 缺少 runtime_core');
  }
  if (!String(runtimeCore.target_triple || '').trim()) {
    throw new Error('runtime_core.target_triple 不能为空');
  }
  const artifactPath = assertRuntimeCoreArtifactPath(runtimeCore.artifact_path);
  const artifactAbsolutePath = path.join(stagedRoot, ...artifactPath.split('/'));
  assertRegularFile(artifactAbsolutePath, 'runtime core artifact');
  if (sha256File(artifactAbsolutePath) !== runtimeCore.artifact_sha256) {
    throw new Error('runtime_core.artifact_sha256 与 archive artifact 不匹配');
  }
  if (runtimeCore.gpl_license_notice_path !== RUNTIME_CORE_LICENSE_NOTICE_PATH) {
    throw new Error('runtime_core.gpl_license_notice_path 无效');
  }
  const licensePath = path.join(
    stagedRoot,
    ...RUNTIME_CORE_LICENSE_NOTICE_PATH.split('/')
  );
  assertRegularFile(licensePath, 'runtime core GPL license notice');
  const licenseNotice = fs.readFileSync(licensePath, 'utf8');
  assertGplLicenseNotice(licenseNotice);
  if (sha256File(licensePath) !== runtimeCore.gpl_license_notice_sha256) {
    throw new Error('runtime_core.gpl_license_notice_sha256 与 archive notice 不匹配');
  }
  assertCorrespondingSource(runtimeCore.corresponding_source);

  return release;
}

module.exports = {
  payloadSha256,
  verifySignedRuntimeCoreRelease,
  writeRuntimeCoreComplianceMetadata,
  writeOfficialSignatureFiles,
};
