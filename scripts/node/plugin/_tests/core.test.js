const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');

const { main, startDemoServer } = require('../core.js');

function makeTempPluginPath(prefix = 'oneflowbase-plugin-') {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  return path.join(tempDir, 'acme-openai-compatible');
}

function request(url) {
  return new Promise((resolve, reject) => {
    const req = http.get(url, (response) => {
      let body = '';
      response.setEncoding('utf8');
      response.on('data', (chunk) => {
        body += chunk;
      });
      response.on('end', () => {
        resolve({
          statusCode: response.statusCode,
          headers: response.headers,
          body,
        });
      });
    });

    req.on('error', reject);
  });
}

function compareStablePath(left, right) {
  if (left === right) {
    return 0;
  }
  return left < right ? -1 : 1;
}

function payloadSha256(rootDir) {
  const entries = [];

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

      entries.push([relativePath, fs.readFileSync(absolutePath)]);
    }
  }

  walk(rootDir);
  entries.sort((left, right) => compareStablePath(left[0], right[0]));

  const hasher = crypto.createHash('sha256');
  for (const [relativePath, content] of entries) {
    hasher.update(relativePath);
    hasher.update(Buffer.from([0]));
    hasher.update(content);
    hasher.update(Buffer.from([0]));
  }

  return `sha256:${hasher.digest('hex')}`;
}

function writeFakeRuntimeBinary(outputDir, fileName = 'acme_openai_compatible-provider') {
  const binaryPath = path.join(outputDir, fileName);
  fs.writeFileSync(binaryPath, '#!/usr/bin/env bash\nexit 0\n', 'utf8');
  fs.chmodSync(binaryPath, 0o755);
  return binaryPath;
}

function createFakeTarBin(binDir) {
  const tarScript = path.join(binDir, 'tar');
  const tarSource = `#!/usr/bin/env node
const fs = require('node:fs');

const logPath = process.env.ONEFLOWBASE_FAKE_TAR_LOG;
if (!logPath) {
  process.stderr.write('ONEFLOWBASE_FAKE_TAR_LOG is required\\n');
  process.exit(2);
}

fs.writeFileSync(
  logPath,
  JSON.stringify(
    {
      cwd: process.cwd(),
      args: process.argv.slice(2),
    },
    null,
    2
  )
);

process.stdout.write(Buffer.from(process.env.ONEFLOWBASE_FAKE_TAR_BYTES_B64 || '', 'base64'));
`;

  fs.writeFileSync(tarScript, tarSource, 'utf8');
  fs.chmodSync(tarScript, 0o755);
  return tarScript;
}

test('plugin init scaffolds rust provider source and executable manifest', async () => {
  const pluginPath = makeTempPluginPath();

  await main(['init', pluginPath]);

  const manifest = fs.readFileSync(path.join(pluginPath, 'manifest.yaml'), 'utf8');
  assert.match(manifest, /manifest_version: 1/);
  assert.match(manifest, /plugin_id: acme_openai_compatible/);
  assert.match(manifest, /version: 0\.1\.0/);
  assert.match(manifest, /vendor: 1flowbase/);
  assert.match(manifest, /display_name: acme-openai-compatible/);
  assert.match(
    manifest,
    /description: OpenAI-compatible provider runtime extension/
  );
  assert.match(manifest, /source_kind: official_registry/);
  assert.match(manifest, /trust_level: verified_official/);
  assert.match(manifest, /consumption_kind: runtime_extension/);
  assert.match(manifest, /execution_mode: process_per_call/);
  assert.match(manifest, /slot_codes:\n  - model_provider/);
  assert.match(manifest, /binding_targets:\n  - workspace/);
  assert.match(manifest, /selection_mode: assignment_then_select/);
  assert.match(manifest, /minimum_host_version: 0\.1\.0/);
  assert.match(manifest, /contract_version: 1flowbase\.provider\/v2/);
  assert.match(manifest, /schema_version: 1flowbase\.plugin\.manifest\/v1/);
  assert.match(manifest, /protocol: stdio_json/);
  assert.match(manifest, /entry: bin\/acme_openai_compatible-provider/);
  assert.match(manifest, /timeout_ms: 30000/);
  assert.match(manifest, /memory_bytes: 268435456/);
  assert.match(manifest, /node_contributions: \[\]/);
  assert.equal(fs.existsSync(path.join(pluginPath, 'Cargo.toml')), true);
  assert.equal(fs.existsSync(path.join(pluginPath, 'src', 'main.rs')), true);
  const rustMain = fs.readFileSync(path.join(pluginPath, 'src', 'main.rs'), 'utf8');
  assert.doesNotMatch(rustMain, /\.unwrap\(/);
  assert.match(rustMain, /eprintln!\("failed to read stdin: \{\}"/);
  assert.match(rustMain, /std::process::exit\(1\)/);
  assert.equal(fs.existsSync(path.join(pluginPath, 'i18n', 'en_US.json')), true);
  assert.equal(fs.existsSync(path.join(pluginPath, 'i18n', 'zh_Hans.json')), true);
  assert.equal(
    fs.existsSync(path.join(pluginPath, 'provider', 'acme_openai_compatible.yaml')),
    true
  );
  assert.equal(
    fs.existsSync(path.join(pluginPath, 'provider', 'acme_openai_compatible.js')),
    false
  );

  const zhHansI18n = JSON.parse(
    fs.readFileSync(path.join(pluginPath, 'i18n', 'zh_Hans.json'), 'utf8')
  );
  assert.equal(zhHansI18n.plugin.label, 'acme-openai-compatible');
  assert.equal(zhHansI18n.provider.label, 'acme-openai-compatible');
  assert.match(zhHansI18n.plugin.description, /运行时扩展/);
});

test('plugin demo init writes demo assets and helper config files', async () => {
  const pluginPath = makeTempPluginPath();

  await main(['init', pluginPath]);
  await main(['demo', 'init', pluginPath]);

  const indexHtml = path.join(pluginPath, 'demo', 'index.html');
  const appJs = path.join(pluginPath, 'demo', 'app.js');
  const stylesCss = path.join(pluginPath, 'demo', 'styles.css');
  const helperConfig = path.join(pluginPath, 'scripts', 'demo.runner.example.json');

  assert.equal(fs.existsSync(indexHtml), true);
  assert.equal(fs.existsSync(appJs), true);
  assert.equal(fs.existsSync(stylesCss), true);
  assert.equal(fs.existsSync(helperConfig), true);

  const html = fs.readFileSync(indexHtml, 'utf8');
  assert.match(html, /Provider Instance/);
  assert.match(html, /Validate/);
  assert.match(html, /List Models/);
  assert.match(html, /Prompt \/ Stream/);
  assert.match(html, /Tool Call \/ MCP/);
  assert.match(html, /Usage \/ Token/);
});

test('plugin demo dev serves static demo assets and injected runtime config', async () => {
  const pluginPath = makeTempPluginPath();

  await main(['init', pluginPath]);
  await main(['demo', 'init', pluginPath]);

  const serverHandle = await startDemoServer({
    pluginPath,
    host: '127.0.0.1',
    port: 0,
    runnerUrl: 'http://127.0.0.1:7801',
    silent: true,
  });

  let config;
  try {
    const indexResponse = await request(`${serverHandle.baseUrl}/`);
    assert.equal(indexResponse.statusCode, 200);
    assert.match(indexResponse.body, /1flowbase Plugin Demo/);

    const configResponse = await request(`${serverHandle.baseUrl}/__plugin_demo_config`);
    assert.equal(configResponse.statusCode, 200);

    config = JSON.parse(configResponse.body);
    assert.equal(config.runnerUrl, 'http://127.0.0.1:7801');
    assert.equal(config.providerCode, 'acme_openai_compatible');
    assert.notEqual(config.packageRoot, pluginPath);
    assert.equal(fs.existsSync(path.join(config.packageRoot, 'manifest.yaml')), true);
    assert.equal(fs.existsSync(path.join(config.packageRoot, 'provider')), true);
    assert.equal(fs.existsSync(path.join(config.packageRoot, 'demo')), false);
    assert.equal(fs.existsSync(path.join(config.packageRoot, 'scripts')), false);
  } finally {
    await serverHandle.close();
  }

  assert.equal(fs.existsSync(config.packageRoot), false);
});

test('plugin demo dev rejects target without generated demo assets', async () => {
  const pluginPath = makeTempPluginPath();

  await main(['init', pluginPath]);

  await assert.rejects(
    startDemoServer({
      pluginPath,
      host: '127.0.0.1',
      port: 0,
      runnerUrl: 'http://127.0.0.1:7801',
      silent: true,
    }),
    /缺少 demo 资源/
  );
});

test('plugin package copies a target binary into bin and encodes the target in the asset name', async () => {
  const pluginPath = makeTempPluginPath();
  const outputDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-dist-'));

  await main(['init', pluginPath]);
  const fakeBinary = writeFakeRuntimeBinary(outputDir);

  const result = await main([
    'package',
    pluginPath,
    '--out',
    outputDir,
    '--runtime-binary',
    fakeBinary,
    '--target',
    'x86_64-unknown-linux-musl',
  ]);

  assert.match(result.packageFile, /@linux-amd64@[a-f0-9]{64}\.1flowbasepkg$/);
  assert.match(result.checksum, /^[a-f0-9]{64}$/);
  assert.equal(fs.existsSync(result.packageFile), true);

  const extractedDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-extract-'));
  const unpack = spawnSync('tar', ['-xzf', result.packageFile, '-C', extractedDir]);
  assert.equal(unpack.status, 0);
  assert.equal(
    fs.existsSync(path.join(extractedDir, 'bin', 'acme_openai_compatible-provider')),
    true
  );
});

test('plugin package writes a windows executable and asset suffix', async () => {
  const pluginPath = makeTempPluginPath();
  const outputDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-dist-'));

  await main(['init', pluginPath]);

  const runtimeBinary = path.join(outputDir, 'acme_openai_compatible-provider.exe');
  fs.mkdirSync(path.dirname(runtimeBinary), { recursive: true });
  fs.writeFileSync(runtimeBinary, 'echo demo');

  const result = await main([
    'package',
    pluginPath,
    '--out',
    outputDir,
    '--runtime-binary',
    runtimeBinary,
    '--target',
    'x86_64-pc-windows-msvc',
  ]);

  assert.match(result.packageFile, /@windows-amd64@[a-f0-9]{64}\.1flowbasepkg$/);
  assert.ok(fs.readdirSync(outputDir).some((name) => name.includes('@windows-amd64@')));
});

test('plugin package streams tar output into the archive file instead of passing the output path to tar', async () => {
  const pluginPath = makeTempPluginPath();
  const outputDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-dist-'));
  const fakeTarDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-fake-tar-'));
  const tarLogPath = path.join(outputDir, 'fake-tar-log.json');
  const archiveBytes = Buffer.from('fake plugin archive bytes\n', 'utf8');

  await main(['init', pluginPath]);
  createFakeTarBin(fakeTarDir);

  const runtimeBinary = writeFakeRuntimeBinary(outputDir);
  const originalPath = process.env.PATH;
  const originalTarLog = process.env.ONEFLOWBASE_FAKE_TAR_LOG;
  const originalTarBytes = process.env.ONEFLOWBASE_FAKE_TAR_BYTES_B64;

  process.env.PATH = `${fakeTarDir}${path.delimiter}${originalPath || ''}`;
  process.env.ONEFLOWBASE_FAKE_TAR_LOG = tarLogPath;
  process.env.ONEFLOWBASE_FAKE_TAR_BYTES_B64 = archiveBytes.toString('base64');

  try {
    const result = await main([
      'package',
      pluginPath,
      '--out',
      outputDir,
      '--runtime-binary',
      runtimeBinary,
      '--target',
      'x86_64-pc-windows-msvc',
    ]);

    const tarLog = JSON.parse(fs.readFileSync(tarLogPath, 'utf8'));
    const expectedChecksum = crypto.createHash('sha256').update(archiveBytes).digest('hex');

    assert.deepEqual(tarLog.args, ['-czf', '-', '.']);
    assert.match(path.basename(tarLog.cwd), /^1flowbase-plugin-package-/);
    assert.equal(fs.readFileSync(result.packageFile).equals(archiveBytes), true);
    assert.match(
      result.packageFile,
      new RegExp(`@windows-amd64@${expectedChecksum}\\.1flowbasepkg$`)
    );
  } finally {
    if (originalPath === undefined) {
      delete process.env.PATH;
    } else {
      process.env.PATH = originalPath;
    }

    if (originalTarLog === undefined) {
      delete process.env.ONEFLOWBASE_FAKE_TAR_LOG;
    } else {
      process.env.ONEFLOWBASE_FAKE_TAR_LOG = originalTarLog;
    }

    if (originalTarBytes === undefined) {
      delete process.env.ONEFLOWBASE_FAKE_TAR_BYTES_B64;
    } else {
      process.env.ONEFLOWBASE_FAKE_TAR_BYTES_B64 = originalTarBytes;
    }
  }
});

test('plugin package excludes demo and scripts from the packaged artifact', async () => {
  const pluginPath = makeTempPluginPath();
  const outputDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-dist-'));

  await main(['init', pluginPath]);
  await main(['demo', 'init', pluginPath]);
  const fakeBinary = writeFakeRuntimeBinary(outputDir);

  const result = await main([
    'package',
    pluginPath,
    '--out',
    outputDir,
    '--runtime-binary',
    fakeBinary,
    '--target',
    'x86_64-unknown-linux-musl',
  ]);
  const extractedDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-extract-'));
  const unpack = spawnSync('tar', ['-xzf', result.packageFile, '-C', extractedDir]);

  assert.equal(unpack.status, 0);
  assert.equal(fs.existsSync(path.join(pluginPath, 'demo')), true);
  assert.equal(fs.existsSync(path.join(pluginPath, 'scripts')), true);
  assert.equal(fs.existsSync(path.join(extractedDir, 'demo')), false);
  assert.equal(fs.existsSync(path.join(extractedDir, 'scripts')), false);
});

test('plugin package excludes rust source files from the packaged artifact', async () => {
  const pluginPath = makeTempPluginPath();
  const outputDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-dist-'));

  await main(['init', pluginPath]);
  const fakeBinary = writeFakeRuntimeBinary(outputDir);

  const result = await main([
    'package',
    pluginPath,
    '--out',
    outputDir,
    '--runtime-binary',
    fakeBinary,
    '--target',
    'x86_64-unknown-linux-musl',
  ]);
  const extractedDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-extract-'));
  const unpack = spawnSync('tar', ['-xzf', result.packageFile, '-C', extractedDir]);

  assert.equal(unpack.status, 0);
  assert.equal(fs.existsSync(path.join(extractedDir, 'manifest.yaml')), true);
  assert.equal(
    fs.existsSync(path.join(extractedDir, 'provider', 'acme_openai_compatible.yaml')),
    true
  );
  assert.equal(fs.existsSync(path.join(extractedDir, 'i18n', 'en_US.json')), true);
  assert.equal(
    fs.existsSync(path.join(extractedDir, 'bin', 'acme_openai_compatible-provider')),
    true
  );
  assert.equal(fs.existsSync(path.join(extractedDir, 'Cargo.toml')), false);
  assert.equal(fs.existsSync(path.join(extractedDir, 'Cargo.lock')), false);
  assert.equal(fs.existsSync(path.join(extractedDir, 'src')), false);
});

test('plugin package writes official signature metadata when signing inputs are provided', async () => {
  const pluginPath = makeTempPluginPath();
  const outputDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-dist-'));
  const extractedDir = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-plugin-extract-'));
  const signingKeyFile = path.join(outputDir, 'official-signing-key.pem');
  const { privateKey, publicKey } = crypto.generateKeyPairSync('ed25519');

  await main(['init', pluginPath]);
  const fakeBinary = writeFakeRuntimeBinary(outputDir);
  fs.writeFileSync(
    signingKeyFile,
    privateKey.export({ format: 'pem', type: 'pkcs8' }),
    'utf8'
  );

  const result = await main([
    'package',
    pluginPath,
    '--out',
    outputDir,
    '--runtime-binary',
    fakeBinary,
    '--target',
    'x86_64-unknown-linux-musl',
    '--signing-key-pem-file',
    signingKeyFile,
    '--signing-key-id',
    'official-key-2026-04',
    '--issued-at',
    '2026-04-19T13:00:00Z',
  ]);

  const unpack = spawnSync('tar', ['-xzf', result.packageFile, '-C', extractedDir]);
  assert.equal(unpack.status, 0);

  const releasePath = path.join(extractedDir, '_meta', 'official-release.json');
  const signaturePath = path.join(extractedDir, '_meta', 'official-release.sig');
  assert.equal(fs.existsSync(releasePath), true);
  assert.equal(fs.existsSync(signaturePath), true);

  const releaseBytes = fs.readFileSync(releasePath);
  const release = JSON.parse(releaseBytes.toString('utf8'));
  const signature = fs.readFileSync(signaturePath);

  assert.equal(release.schema_version, 1);
  assert.equal(release.plugin_id, 'acme_openai_compatible');
  assert.equal(release.provider_code, 'acme_openai_compatible');
  assert.equal(release.version, '0.1.0');
  assert.equal(release.contract_version, '1flowbase.provider/v2');
  assert.equal(release.signature_algorithm, 'ed25519');
  assert.equal(release.signing_key_id, 'official-key-2026-04');
  assert.equal(release.issued_at, '2026-04-19T13:00:00Z');
  assert.equal(release.payload_sha256, payloadSha256(extractedDir));
  assert.equal(
    crypto.verify(null, releaseBytes, publicKey, signature),
    true
  );
});
