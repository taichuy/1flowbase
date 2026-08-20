const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const { createArtifactRoot } = require('../fs.js');
const { readPluginCode } = require('../manifest.js');
const { createPluginPackage, parseRustTargetTriple } = require('../package.js');
const {
  payloadSha256,
  verifySignedRuntimeCoreRelease,
} = require('../release.js');
const { createPluginScaffold } = require('../init.js');
const { spawnSync } = require('node:child_process');
const crypto = require('node:crypto');

test('readPluginCode prefers plugin_id from manifest', () => {
  const pluginPath = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-manifest-'));
  fs.writeFileSync(
    path.join(pluginPath, 'manifest.yaml'),
    'plugin_id: acme_provider\nversion: 0.2.0\n',
    'utf8'
  );

  assert.equal(readPluginCode(pluginPath), 'acme_provider');
});

test('readPluginCode ignores legacy plugin_code fallback once plugin_id is the only supported manifest key', () => {
  const pluginPath = path.join(
    fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-manifest-no-legacy-')),
    'acme-openai-compatible'
  );
  fs.mkdirSync(pluginPath, { recursive: true });
  fs.writeFileSync(
    path.join(pluginPath, 'manifest.yaml'),
    'plugin_code: legacy_provider_code\nversion: 0.2.0\n',
    'utf8'
  );

  assert.equal(readPluginCode(pluginPath), 'acme_openai_compatible');
});

test('createArtifactRoot excludes requested top-level entries', () => {
  const pluginPath = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-artifact-'));
  fs.writeFileSync(path.join(pluginPath, 'manifest.yaml'), 'plugin_id: acme_provider\n', 'utf8');
  fs.mkdirSync(path.join(pluginPath, 'demo'), { recursive: true });
  fs.writeFileSync(path.join(pluginPath, 'demo', 'index.html'), '<h1>demo</h1>', 'utf8');
  fs.mkdirSync(path.join(pluginPath, 'provider'), { recursive: true });
  fs.writeFileSync(path.join(pluginPath, 'provider', 'acme_provider.yaml'), 'provider_code: acme_provider\n', 'utf8');

  const artifactRoot = createArtifactRoot(pluginPath, {
    excludedEntries: ['demo'],
    prefix: 'oneflowbase-plugin-artifact-test',
  });

  assert.equal(fs.existsSync(path.join(artifactRoot, 'manifest.yaml')), true);
  assert.equal(fs.existsSync(path.join(artifactRoot, 'provider', 'acme_provider.yaml')), true);
  assert.equal(fs.existsSync(path.join(artifactRoot, 'demo')), false);
});

test('parseRustTargetTriple returns expected asset suffix for windows target', () => {
  assert.deepEqual(parseRustTargetTriple('x86_64-pc-windows-msvc'), {
    rustTargetTriple: 'x86_64-pc-windows-msvc',
    os: 'windows',
    arch: 'amd64',
    libc: 'msvc',
    assetSuffix: 'windows-amd64',
    executableSuffix: '.exe',
  });
});

test('payloadSha256 ignores release metadata under _meta', () => {
  const rootDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-release-'));
  fs.mkdirSync(path.join(rootDir, '_meta'), { recursive: true });
  fs.mkdirSync(path.join(rootDir, 'provider'), { recursive: true });
  fs.writeFileSync(path.join(rootDir, 'provider', 'acme_provider.yaml'), 'provider_code: acme_provider\n', 'utf8');
  fs.writeFileSync(path.join(rootDir, '_meta', 'official-release.json'), '{"schema_version":1}', 'utf8');

  const baseline = payloadSha256(rootDir);
  fs.writeFileSync(path.join(rootDir, '_meta', 'official-release.sig'), 'signature-bytes', 'utf8');

  assert.equal(payloadSha256(rootDir), baseline);
});

test('AC-006/AC-014: signed package archives bind a target runtime core binary to GPL notice and corresponding source', () => {
  const workspace = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-runtime-core-'));
  const pluginPath = path.join(workspace, 'acme-provider');
  const outputDir = path.join(workspace, 'dist');
  const runtimeBinary = path.join(workspace, 'acme-provider-wrapper');
  const runtimeCoreBinary = path.join(workspace, '1flowbase-runtime-core');
  const licenseNotice = path.join(workspace, 'COPYING');
  const signingKeyFile = path.join(workspace, 'official-signing-key.pem');
  const extractedDir = path.join(workspace, 'extracted');
  const { privateKey, publicKey } = crypto.generateKeyPairSync('ed25519');

  createPluginScaffold(pluginPath);
  fs.mkdirSync(outputDir, { recursive: true });
  fs.writeFileSync(runtimeBinary, '#!/usr/bin/env sh\nexit 0\n', 'utf8');
  fs.chmodSync(runtimeBinary, 0o755);
  fs.writeFileSync(runtimeCoreBinary, '#!/usr/bin/env sh\necho runtime-core\n', 'utf8');
  fs.chmodSync(runtimeCoreBinary, 0o755);
  fs.writeFileSync(
    licenseNotice,
    'GNU GENERAL PUBLIC LICENSE\nVersion 3, 29 June 2007\n',
    'utf8'
  );
  fs.writeFileSync(
    signingKeyFile,
    privateKey.export({ format: 'pem', type: 'pkcs8' }),
    'utf8'
  );

  const cli = path.resolve(__dirname, '..', 'cli.js');
  const packageCommand = spawnSync(
    process.execPath,
    [
      cli,
      'package',
      pluginPath,
      '--out',
      outputDir,
      '--runtime-binary',
      runtimeBinary,
      '--target',
      'x86_64-unknown-linux-musl',
      '--runtime-core-binary',
      runtimeCoreBinary,
      '--signing-key-pem-file',
      signingKeyFile,
      '--signing-key-id',
      'official-key-2026-04',
      '--issued-at',
      '2026-08-20T00:00:00Z',
      '--runtime-core-gpl-license-notice',
      licenseNotice,
      '--runtime-core-corresponding-source',
      'https://example.test/acme-provider/source/v0.1.0',
    ],
    { encoding: 'utf8' }
  );
  assert.equal(packageCommand.status, 0, packageCommand.stderr);
  const packageFile = path.join(
    outputDir,
    fs.readdirSync(outputDir).find((name) => name.endsWith('.1flowbasepkg'))
  );

  const unpack = spawnSync('tar', ['-xzf', packageFile, '-C', extractedDir]);
  assert.equal(unpack.status, 0);
  const releasePath = path.join(extractedDir, '_meta', 'official-release.json');
  const signaturePath = path.join(extractedDir, '_meta', 'official-release.sig');
  const releaseBytes = fs.readFileSync(releasePath);
  const signatureBytes = fs.readFileSync(signaturePath);
  const release = JSON.parse(releaseBytes.toString('utf8'));
  assert.deepEqual(release.runtime_core, {
    target_triple: 'x86_64-unknown-linux-musl',
    artifact_path: 'runtime-core/x86_64-unknown-linux-musl/1flowbase-runtime-core',
    artifact_sha256: `sha256:${crypto.createHash('sha256').update(fs.readFileSync(runtimeCoreBinary)).digest('hex')}`,
    gpl_license_notice_path: '_meta/runtime-core-gpl-license-notice.txt',
    gpl_license_notice_sha256: `sha256:${crypto.createHash('sha256').update(fs.readFileSync(licenseNotice)).digest('hex')}`,
    corresponding_source: 'https://example.test/acme-provider/source/v0.1.0',
  });
  assert.equal(
    fs.readFileSync(
      path.join(extractedDir, '_meta', 'runtime-core-gpl-license-notice.txt'),
      'utf8'
    ),
    fs.readFileSync(licenseNotice, 'utf8')
  );
  assert.equal(
    fs.readFileSync(path.join(extractedDir, 'manifest.yaml'), 'utf8').includes(
      'entry: bin/acme_provider-provider'
    ),
    true
  );
  assert.equal(
    fs.readFileSync(path.join(extractedDir, 'bin', 'acme_provider-provider'), 'utf8'),
    fs.readFileSync(runtimeBinary, 'utf8')
  );
  assert.equal(
    verifySignedRuntimeCoreRelease(extractedDir, publicKey),
    release
  );

  fs.appendFileSync(
    path.join(
      extractedDir,
      'runtime-core',
      'x86_64-unknown-linux-musl',
      '1flowbase-runtime-core'
    ),
    'tamper'
  );
  assert.throws(
    () => verifySignedRuntimeCoreRelease(extractedDir, publicKey),
    /runtime_core\.artifact_sha256/
  );

  fs.copyFileSync(
    runtimeCoreBinary,
    path.join(
      extractedDir,
      'runtime-core',
      'x86_64-unknown-linux-musl',
      '1flowbase-runtime-core'
    )
  );
  const mismatchedReceipt = {
    ...release,
    runtime_core: {
      ...release.runtime_core,
      artifact_sha256: 'sha256:0000000000000000000000000000000000000000000000000000000000000000',
    },
  };
  const mismatchedReceiptBytes = Buffer.from(JSON.stringify(mismatchedReceipt), 'utf8');
  fs.writeFileSync(releasePath, mismatchedReceiptBytes);
  fs.writeFileSync(signaturePath, crypto.sign(null, mismatchedReceiptBytes, privateKey));
  assert.throws(
    () => verifySignedRuntimeCoreRelease(extractedDir, publicKey),
    /runtime_core\.artifact_sha256/
  );

  fs.writeFileSync(releasePath, releaseBytes);
  fs.writeFileSync(signaturePath, signatureBytes);
  fs.writeFileSync(
    path.join(extractedDir, '_meta', 'runtime-core-gpl-license-notice.txt'),
    'GNU GENERAL PUBLIC LICENSE\nVersion 3, altered\n',
    'utf8'
  );
  assert.throws(
    () => verifySignedRuntimeCoreRelease(extractedDir, publicKey),
    /runtime_core\.gpl_license_notice_sha256/
  );

  const missingReceipt = { ...release };
  delete missingReceipt.runtime_core;
  const missingReceiptBytes = Buffer.from(JSON.stringify(missingReceipt), 'utf8');
  fs.writeFileSync(releasePath, missingReceiptBytes);
  fs.writeFileSync(signaturePath, crypto.sign(null, missingReceiptBytes, privateKey));
  assert.throws(
    () => verifySignedRuntimeCoreRelease(extractedDir, publicKey),
    /缺少 runtime_core/
  );
});

test('signed runtime core packaging rejects missing or invalid GPL/source compliance inputs', () => {
  const workspace = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-runtime-core-invalid-'));
  const pluginPath = path.join(workspace, 'acme-provider');
  const outputDir = path.join(workspace, 'dist');
  const runtimeBinary = path.join(workspace, 'acme-provider');
  const runtimeCoreBinary = path.join(workspace, '1flowbase-runtime-core');
  const signingKeyFile = path.join(workspace, 'official-signing-key.pem');
  const invalidNotice = path.join(workspace, 'NOTICE');
  const gplNotice = path.join(workspace, 'COPYING');
  const { privateKey } = crypto.generateKeyPairSync('ed25519');

  createPluginScaffold(pluginPath);
  fs.mkdirSync(outputDir, { recursive: true });
  fs.writeFileSync(runtimeBinary, 'runtime core', 'utf8');
  fs.writeFileSync(runtimeCoreBinary, 'runtime core binary', 'utf8');
  fs.writeFileSync(invalidNotice, 'proprietary notice', 'utf8');
  fs.writeFileSync(gplNotice, 'GPL-3.0-or-later\n', 'utf8');
  fs.writeFileSync(signingKeyFile, privateKey.export({ format: 'pem', type: 'pkcs8' }), 'utf8');
  const baseOptions = {
    runtimeBinaryFile: runtimeBinary,
    runtimeCoreBinaryFile: runtimeCoreBinary,
    targetTriple: 'x86_64-unknown-linux-musl',
    signingKeyPemFile: signingKeyFile,
    signingKeyId: 'official-key-2026-04',
  };

  assert.throws(
    () => createPluginPackage(pluginPath, outputDir, baseOptions),
    /runtime core 签名包需要 GPL license notice/
  );
  assert.throws(
    () =>
      createPluginPackage(pluginPath, outputDir, {
        ...baseOptions,
        runtimeCoreGplLicenseNoticeFile: gplNotice,
        runtimeCoreCorrespondingSource: 'https://example.test/acme-provider/source/v0.1.0',
        runtimeCoreBinaryFile: null,
      }),
    /runtime core 签名包需要 --runtime-core-binary/
  );
  assert.throws(
    () =>
      createPluginPackage(pluginPath, outputDir, {
        ...baseOptions,
        runtimeCoreBinaryFile: runtimeBinary,
        runtimeCoreGplLicenseNoticeFile: gplNotice,
        runtimeCoreCorrespondingSource: 'https://example.test/acme-provider/source/v0.1.0',
      }),
    /必须与 manifest\.runtime\.entry wrapper 使用不同输入文件/
  );
  assert.throws(
    () =>
      createPluginPackage(pluginPath, outputDir, {
        ...baseOptions,
        runtimeCoreGplLicenseNoticeFile: invalidNotice,
        runtimeCoreCorrespondingSource: 'ftp://example.test/source',
      }),
    /GPL license notice 必须明确声明 GNU GPL/
  );
  assert.throws(
    () =>
      createPluginPackage(pluginPath, outputDir, {
        ...baseOptions,
        runtimeCoreGplLicenseNoticeFile: gplNotice,
        runtimeCoreCorrespondingSource: 'ftp://example.test/source',
      }),
    /corresponding source pointer 必须是有效的 HTTPS URL/
  );
});
