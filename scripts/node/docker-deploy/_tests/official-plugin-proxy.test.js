const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');

function readRepoFile(...segments) {
  return fs.readFileSync(path.join(repoRoot, ...segments), 'utf8');
}

function makeExecutable(filePath, content) {
  fs.writeFileSync(filePath, content, { mode: 0o755 });
}

function shellQuote(value) {
  return `'${String(value).replace(/'/gu, "'\\''")}'`;
}

function runInteractiveShellDeploy({ tempRoot, tempBin, input }) {
  return spawnSync(
    'script',
    [
      '-qfec',
      `sh ${shellQuote(path.join(repoRoot, 'scripts', 'shell', 'docker-deploy.sh'))}`,
      '/dev/null',
    ],
    {
      cwd: tempRoot,
      env: {
        ...process.env,
        PATH: `${tempBin}${path.delimiter}${process.env.PATH || ''}`,
      },
      input,
      encoding: 'utf8',
    },
  );
}

test('docker deploy shell script can prefill official plugin GitHub proxy URL', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-docker-deploy-'));
  const tempBin = path.join(tempRoot, 'bin');
  const dockerDir = path.join(tempRoot, 'docker');
  fs.mkdirSync(tempBin);
  fs.mkdirSync(dockerDir);
  fs.writeFileSync(
    path.join(dockerDir, '.env.example'),
    'POSTGRES_PASSWORD=change-me\nAPI_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=\n',
  );
  makeExecutable(
    path.join(tempBin, 'docker'),
    '#!/usr/bin/env sh\nif [ "$1 $2 $3" = "compose version " ]; then exit 0; fi\nexit 0\n',
  );

  const result = spawnSync(
    'sh',
    [
      path.join(repoRoot, 'scripts', 'shell', 'docker-deploy.sh'),
      '--non-interactive',
      '--plugin-github-proxy-url',
      'https://proxy.example/',
    ],
    {
      cwd: tempRoot,
      env: {
        ...process.env,
        PATH: `${tempBin}${path.delimiter}${process.env.PATH || ''}`,
      },
      encoding: 'utf8',
    },
  );

  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(
    fs.readFileSync(path.join(dockerDir, '.env'), 'utf8'),
    /^API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=https:\/\/proxy\.example\/$/mu,
  );
});

test('docker deploy shell script adds default official plugin signature policy to old env files', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-docker-deploy-'));
  const tempBin = path.join(tempRoot, 'bin');
  const dockerDir = path.join(tempRoot, 'docker');
  fs.mkdirSync(tempBin);
  fs.mkdirSync(dockerDir);
  fs.writeFileSync(
    path.join(dockerDir, '.env.example'),
    [
      'POSTGRES_PASSWORD=example-password',
      'API_PROVIDER_SECRET_MASTER_KEY=example-secret',
      'API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=',
      '',
    ].join('\n'),
  );
  fs.writeFileSync(
    path.join(dockerDir, '.env'),
    [
      'POSTGRES_PASSWORD=old-password',
      'API_PROVIDER_SECRET_MASTER_KEY=old-secret',
      'API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=',
      '',
    ].join('\n'),
  );
  makeExecutable(
    path.join(tempBin, 'docker'),
    '#!/usr/bin/env sh\nif [ "$1 $2" = "compose version" ]; then exit 0; fi\nexit 0\n',
  );

  const result = spawnSync(
    'sh',
    [
      path.join(repoRoot, 'scripts', 'shell', 'docker-deploy.sh'),
      '--non-interactive',
      '--no-pull',
      '--no-start',
    ],
    {
      cwd: tempRoot,
      env: {
        ...process.env,
        PATH: `${tempBin}${path.delimiter}${process.env.PATH || ''}`,
      },
      encoding: 'utf8',
    },
  );

  const envFile = fs.readFileSync(path.join(dockerDir, '.env'), 'utf8');
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(result.stdout, /Added API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED=true to docker\/.env/u);
  assert.match(envFile, /^API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED=true$/mu);
});

test('docker deploy shell script can prefill official plugin signature policy', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-docker-deploy-'));
  const tempBin = path.join(tempRoot, 'bin');
  const dockerDir = path.join(tempRoot, 'docker');
  fs.mkdirSync(tempBin);
  fs.mkdirSync(dockerDir);
  fs.writeFileSync(
    path.join(dockerDir, '.env.example'),
    [
      'POSTGRES_PASSWORD=example-password',
      'API_PROVIDER_SECRET_MASTER_KEY=example-secret',
      'API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=',
      'API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED=true',
      '',
    ].join('\n'),
  );
  makeExecutable(
    path.join(tempBin, 'docker'),
    '#!/usr/bin/env sh\nif [ "$1 $2" = "compose version" ]; then exit 0; fi\nexit 0\n',
  );

  const result = spawnSync(
    'sh',
    [
      path.join(repoRoot, 'scripts', 'shell', 'docker-deploy.sh'),
      '--non-interactive',
      '--no-pull',
      '--no-start',
      '--official-plugin-signature-required',
      'false',
    ],
    {
      cwd: tempRoot,
      env: {
        ...process.env,
        PATH: `${tempBin}${path.delimiter}${process.env.PATH || ''}`,
      },
      encoding: 'utf8',
    },
  );

  const envFile = fs.readFileSync(path.join(dockerDir, '.env'), 'utf8');
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(result.stdout, /Updated API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED in docker\/.env/u);
  assert.match(envFile, /^API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED=false$/mu);
});

test('docker deploy shell script generates provider secret when env example keeps placeholder', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-docker-deploy-'));
  const tempBin = path.join(tempRoot, 'bin');
  const dockerDir = path.join(tempRoot, 'docker');
  fs.mkdirSync(tempBin);
  fs.mkdirSync(dockerDir);
  fs.writeFileSync(
    path.join(dockerDir, '.env.example'),
    [
      'FLOWBASE_WEB_VERSION=latest',
      'FLOWBASE_API_SERVER_VERSION=latest',
      'FLOWBASE_PLUGIN_RUNNER_VERSION=latest',
      'POSTGRES_PASSWORD=example-password',
      'BOOTSTRAP_ROOT_PASSWORD=example-root-password',
      'API_PROVIDER_SECRET_MASTER_KEY=change-me-provider-secret-master-key',
      'API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=',
      '',
    ].join('\n'),
  );
  makeExecutable(
    path.join(tempBin, 'docker'),
    '#!/usr/bin/env sh\nif [ "$1 $2" = "compose version" ]; then exit 0; fi\nexit 0\n',
  );

  const result = spawnSync(
    'sh',
    [
      path.join(repoRoot, 'scripts', 'shell', 'docker-deploy.sh'),
      '--non-interactive',
      '--no-pull',
      '--no-start',
    ],
    {
      cwd: tempRoot,
      env: {
        ...process.env,
        PATH: `${tempBin}${path.delimiter}${process.env.PATH || ''}`,
      },
      encoding: 'utf8',
    },
  );

  const envFile = fs.readFileSync(path.join(dockerDir, '.env'), 'utf8');
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(result.stdout, /Generated API_PROVIDER_SECRET_MASTER_KEY in docker\/.env/u);
  assert.doesNotMatch(envFile, /^API_PROVIDER_SECRET_MASTER_KEY=change-me-provider-secret-master-key$/mu);
  assert.match(envFile, /^API_PROVIDER_SECRET_MASTER_KEY=[0-9a-f]{64}$/mu);
});

test('docker deploy shell script shows and updates existing env configuration', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-docker-deploy-'));
  const tempBin = path.join(tempRoot, 'bin');
  const dockerDir = path.join(tempRoot, 'docker');
  fs.mkdirSync(tempBin);
  fs.mkdirSync(dockerDir);
  fs.writeFileSync(
    path.join(dockerDir, '.env.example'),
    [
      'FLOWBASE_WEB_VERSION=latest',
      'FLOWBASE_API_SERVER_VERSION=latest',
      'FLOWBASE_PLUGIN_RUNNER_VERSION=latest',
      'POSTGRES_PASSWORD=example-password',
      'BOOTSTRAP_ROOT_ACCOUNT=example-root',
      'BOOTSTRAP_ROOT_PASSWORD=example-root-password',
      'API_PROVIDER_SECRET_MASTER_KEY=example-secret',
      'WEB_PORT=4100',
      'API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=',
      '',
    ].join('\n'),
  );
  fs.writeFileSync(
    path.join(dockerDir, '.env'),
    [
      'FLOWBASE_WEB_VERSION=latest',
      'FLOWBASE_API_SERVER_VERSION=latest',
      'FLOWBASE_PLUGIN_RUNNER_VERSION=latest',
      'POSTGRES_PASSWORD=old-password',
      'BOOTSTRAP_ROOT_ACCOUNT=old-root',
      'BOOTSTRAP_ROOT_PASSWORD=old-root-password',
      'API_PROVIDER_SECRET_MASTER_KEY=old-secret',
      'WEB_PORT=3000',
      'API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=https://old.example/',
      '',
    ].join('\n'),
  );
  makeExecutable(
    path.join(tempBin, 'docker'),
    `#!/usr/bin/env sh
if [ "$1 $2" = "compose version" ]; then exit 0; fi
if [ "$1 $2" = "image inspect" ]; then exit 1; fi
if [ "$1" = "info" ]; then exit 0; fi
if [ "$1 $2 $3 $4" = "compose up -d db" ]; then exit 0; fi
if [ "$1 $2 $3" = "compose exec -T" ]; then exit 0; fi
exit 0
`,
  );

  const keepResult = runInteractiveShellDeploy({
    tempRoot,
    tempBin,
    input: `n\nn\nn\n`,
  });
  assert.equal(keepResult.status, 0, `${keepResult.stdout}\n${keepResult.stderr}`);
  assert.match(keepResult.stdout, /Current docker\/.env configuration:/u);
  assert.match(keepResult.stdout, /POSTGRES_PASSWORD=<set>/u);
  assert.match(keepResult.stdout, /Update current docker\/.env configuration\?/u);
  assert.match(fs.readFileSync(path.join(dockerDir, '.env'), 'utf8'), /^POSTGRES_PASSWORD=old-password$/mu);

  const updateResult = runInteractiveShellDeploy({
    tempRoot,
    tempBin,
    input: [
      'y',
      '',
      'new-password',
      '',
      'new-root-password',
      'new-provider-secret',
      '4100',
      'n',
      'n',
      'n',
      '',
      '',
    ].join('\n'),
  });
  assert.equal(updateResult.status, 0, `${updateResult.stdout}\n${updateResult.stderr}`);
  assert.match(
    fs.readFileSync(path.join(dockerDir, '.env'), 'utf8'),
    /^POSTGRES_PASSWORD=new-password$/mu,
  );
  assert.match(fs.readFileSync(path.join(dockerDir, '.env'), 'utf8'), /^WEB_PORT=4100$/mu);
  assert.match(
    fs.readFileSync(path.join(dockerDir, '.env'), 'utf8'),
    /^API_PROVIDER_SECRET_MASTER_KEY=new-provider-secret$/mu,
  );
  assert.doesNotMatch(fs.readFileSync(path.join(dockerDir, '.env'), 'utf8'), /old-password/u);
});

test('docker deploy shell script syncs postgres password when existing pgdata is reused', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-docker-deploy-'));
  const tempBin = path.join(tempRoot, 'bin');
  const dockerDir = path.join(tempRoot, 'docker');
  const callsFile = path.join(tempRoot, 'docker-calls.log');
  fs.mkdirSync(tempBin);
  fs.mkdirSync(path.join(dockerDir, 'postgres', 'data', 'pgdata'), { recursive: true });
  fs.writeFileSync(path.join(dockerDir, 'postgres', 'data', 'pgdata', 'PG_VERSION'), '16\n');
  fs.writeFileSync(
    path.join(dockerDir, '.env.example'),
    [
      'POSTGRES_DB=1flowbase',
      'POSTGRES_USER=postgres',
      'POSTGRES_PASSWORD=example-password',
      'API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=',
      '',
    ].join('\n'),
  );
  fs.writeFileSync(
    path.join(dockerDir, '.env'),
    [
      'POSTGRES_DB=1flowbase',
      'POSTGRES_USER=postgres',
      'POSTGRES_PASSWORD=old-password',
      'API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=',
      '',
    ].join('\n'),
  );
  makeExecutable(
    path.join(tempBin, 'docker'),
    `#!/usr/bin/env sh
printf '%s\\n' "$*" >> ${shellQuote(callsFile)}
if [ "$1 $2" = "compose version" ]; then exit 0; fi
if [ "$1" = "info" ]; then exit 0; fi
if [ "$1 $2 $3 $4" = "compose up -d db" ]; then exit 0; fi
if [ "$1 $2 $3" = "compose exec -T" ]; then exit 0; fi
exit 0
`,
  );

  const result = spawnSync(
    'sh',
    [
      path.join(repoRoot, 'scripts', 'shell', 'docker-deploy.sh'),
      '--non-interactive',
      '--db-password',
      "new'password",
    ],
    {
      cwd: tempRoot,
      env: {
        ...process.env,
        PATH: `${tempBin}${path.delimiter}${process.env.PATH || ''}`,
      },
      encoding: 'utf8',
    },
  );

  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(result.stdout, /Postgres password changed and existing pgdata was found/u);
  const calls = fs.readFileSync(callsFile, 'utf8');
  assert.match(calls, /compose up -d db/u);
  assert.match(calls, /ALTER USER postgres WITH PASSWORD 'new''password'/u);
  assert.match(fs.readFileSync(path.join(dockerDir, '.env'), 'utf8'), /^POSTGRES_PASSWORD=new'password$/mu);
});

test('docker deploy shell script can configure and start with external postgres', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-docker-deploy-'));
  const tempBin = path.join(tempRoot, 'bin');
  const dockerDir = path.join(tempRoot, 'docker');
  const callsFile = path.join(tempRoot, 'docker-calls.log');
  fs.mkdirSync(tempBin);
  fs.mkdirSync(dockerDir);
  fs.writeFileSync(
    path.join(dockerDir, '.env.example'),
    [
      'FLOWBASE_WEB_VERSION=latest',
      'FLOWBASE_API_SERVER_VERSION=latest',
      'FLOWBASE_PLUGIN_RUNNER_VERSION=latest',
      'DATABASE_MODE=internal',
      'POSTGRES_DB=1flowbase',
      'POSTGRES_USER=postgres',
      'POSTGRES_PASSWORD=change-me-db-password',
      'API_PROVIDER_SECRET_MASTER_KEY=example-secret',
      'BOOTSTRAP_ROOT_PASSWORD=example-root-password',
      'API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=',
      '',
    ].join('\n'),
  );
  makeExecutable(
    path.join(tempBin, 'docker'),
    `#!/usr/bin/env sh
printf '%s\\n' "$*" >> ${shellQuote(callsFile)}
if [ "$1 $2" = "compose version" ]; then exit 0; fi
if [ "$1" = "info" ]; then
  if [ "$2" = "--format" ]; then
    printf '%s\\n' 'linux/amd64'
  fi
  exit 0
fi
if [ "$1 $2" = "manifest inspect" ]; then
  cat <<'EOF'
{
  "schemaVersion": 2,
  "manifests": [
    { "platform": { "architecture": "amd64", "os": "linux" } }
  ]
}
EOF
  exit 0
fi
if [ "$1 $2" = "compose up" ]; then exit 0; fi
exit 0
`,
  );

  const result = spawnSync(
    'sh',
    [
      path.join(repoRoot, 'scripts', 'shell', 'docker-deploy.sh'),
      '--non-interactive',
      '--no-pull',
      '--start',
      '--database-mode',
      'external',
      '--external-postgres-host',
      'db.internal.example',
      '--external-postgres-port',
      '6543',
      '--external-postgres-db',
      'flowbase_prod',
      '--external-postgres-user',
      'flowbase_user',
      '--external-postgres-password',
      'p@ss:word',
      '--external-postgres-sslmode',
      'require',
      '--cookie-secure',
      'false',
    ],
    {
      cwd: tempRoot,
      env: {
        ...process.env,
        PATH: `${tempBin}${path.delimiter}${process.env.PATH || ''}`,
      },
      encoding: 'utf8',
    },
  );

  const envFile = fs.readFileSync(path.join(dockerDir, '.env'), 'utf8');
  const calls = fs.readFileSync(callsFile, 'utf8');
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(envFile, /^DATABASE_MODE=external$/mu);
  assert.match(envFile, /^EXTERNAL_POSTGRES_HOST=db\.internal\.example$/mu);
  assert.match(envFile, /^EXTERNAL_POSTGRES_PORT=6543$/mu);
  assert.match(envFile, /^EXTERNAL_POSTGRES_DB=flowbase_prod$/mu);
  assert.match(envFile, /^EXTERNAL_POSTGRES_USER=flowbase_user$/mu);
  assert.match(envFile, /^EXTERNAL_POSTGRES_PASSWORD=p@ss:word$/mu);
  assert.match(envFile, /^EXTERNAL_POSTGRES_SSLMODE=require$/mu);
  assert.match(envFile, /^API_COOKIE_SECURE=false$/mu);
  assert.match(
    envFile,
    /^API_DATABASE_URL=postgres:\/\/flowbase_user:p%40ss%3Aword@db\.internal\.example:6543\/flowbase_prod\?sslmode=require$/mu,
  );
  assert.match(calls, /compose -f docker-compose\.external-db\.yaml up -d/u);
  assert.doesNotMatch(calls, /compose up -d db/u);
});

test('docker deploy shell script asks before updating local latest images', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-docker-deploy-'));
  const tempBin = path.join(tempRoot, 'bin');
  const dockerDir = path.join(tempRoot, 'docker');
  fs.mkdirSync(tempBin);
  fs.mkdirSync(dockerDir);
  fs.writeFileSync(
    path.join(dockerDir, '.env.example'),
    [
      'FLOWBASE_WEB_VERSION=latest',
      'FLOWBASE_API_SERVER_VERSION=latest',
      'FLOWBASE_PLUGIN_RUNNER_VERSION=latest',
      'POSTGRES_PASSWORD=example-password',
      'API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=',
      '',
    ].join('\n'),
  );
  fs.writeFileSync(
    path.join(dockerDir, '.env'),
    [
      'FLOWBASE_WEB_VERSION=latest',
      'FLOWBASE_API_SERVER_VERSION=latest',
      'FLOWBASE_PLUGIN_RUNNER_VERSION=latest',
      'POSTGRES_PASSWORD=old-password',
      'API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=',
      '',
    ].join('\n'),
  );
  makeExecutable(
    path.join(tempBin, 'docker'),
    `#!/usr/bin/env sh
if [ "$1 $2" = "compose version" ]; then exit 0; fi
if [ "$1 $2" = "image inspect" ]; then exit 0; fi
if [ "$1 $2" = "compose pull" ]; then
  printf '%s\\n' 'compose pull ran'
  exit 0
fi
if [ "$1" = "info" ]; then
  if [ "$2" = "--format" ]; then
    printf '%s\\n' 'linux/amd64'
  fi
  exit 0
fi
if [ "$1 $2" = "manifest inspect" ]; then
  cat <<'EOF'
{
  "schemaVersion": 2,
  "manifests": [
    { "platform": { "architecture": "amd64", "os": "linux" } }
  ]
}
EOF
  exit 0
fi
exit 0
`,
  );

  const result = runInteractiveShellDeploy({
    tempRoot,
    tempBin,
    input: `n\nn\nn\n${'\n'.repeat(12)}`,
  });

  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(result.stdout, /Local latest Docker images were found\. Update Docker images\?/u);
  assert.doesNotMatch(result.stdout, /Pull Docker images\?/u);
  assert.doesNotMatch(result.stdout, /compose pull ran/u);

  const updateResult = runInteractiveShellDeploy({
    tempRoot,
    tempBin,
    input: `n\ny\nn\n${'\n'.repeat(12)}`,
  });

  assert.equal(updateResult.status, 0, `${updateResult.stdout}\n${updateResult.stderr}`);
  assert.match(updateResult.stdout, /Local latest Docker images were found\. Update Docker images\?/u);
  assert.match(updateResult.stdout, /compose pull ran/u);
});

test('docker compose and env example expose an empty official plugin GitHub proxy URL by default', () => {
  const compose = readRepoFile('docker', 'docker-compose.yaml');
  const envExample = readRepoFile('docker', '.env.example');

  assert.match(
    compose,
    /^\s+API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL: \$\{API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL:-\}$/mu,
  );
  assert.doesNotMatch(compose, /^\s+# API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL:/mu);
  assert.match(envExample, /^API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=$/mu);
  assert.match(compose, /^\s+API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED: \$\{API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED:-true\}$/mu);
  assert.match(envExample, /^API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED=true$/mu);
});

test('docker deploy assets expose external postgres configuration consistently', () => {
  const compose = readRepoFile('docker', 'docker-compose.yaml');
  const externalCompose = readRepoFile('docker', 'docker-compose.external-db.yaml');
  const envExample = readRepoFile('docker', '.env.example');
  const readme = readRepoFile('docker', 'README.md');
  const shellScript = readRepoFile('scripts', 'shell', 'docker-deploy.sh');
  const powershellScript = readRepoFile('scripts', 'powershell', 'docker-deploy.ps1');

  assert.match(
    compose,
    /^\s+API_DATABASE_URL: \$\{API_DATABASE_URL:-postgres:\/\/\$\{POSTGRES_USER:-postgres\}:\$\{POSTGRES_PASSWORD:\?POSTGRES_PASSWORD is required for production compose\}@db:5432\/\$\{POSTGRES_DB:-1flowbase\}\}$/mu,
  );
  assert.match(
    externalCompose,
    /^\s+API_DATABASE_URL: \$\{API_DATABASE_URL:\?API_DATABASE_URL is required for external database compose\}$/mu,
  );
  assert.doesNotMatch(externalCompose, /^\s+db:/mu);

  for (const key of [
    'DATABASE_MODE=internal',
    'API_DATABASE_URL=',
    'EXTERNAL_POSTGRES_HOST=',
    'EXTERNAL_POSTGRES_PORT=5432',
    'EXTERNAL_POSTGRES_DB=1flowbase',
    'EXTERNAL_POSTGRES_USER=postgres',
    'EXTERNAL_POSTGRES_PASSWORD=',
    'EXTERNAL_POSTGRES_SSLMODE=prefer',
    'API_COOKIE_SECURE=true',
  ]) {
    assert.match(envExample, new RegExp(`^${key.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&')}$`, 'mu'));
  }

  for (const script of [shellScript, powershellScript]) {
    assert.match(script, /Database mode/u);
    assert.match(script, /EXTERNAL_POSTGRES_HOST/u);
    assert.match(script, /EXTERNAL_POSTGRES_SSLMODE/u);
    assert.match(script, /API_DATABASE_URL/u);
    assert.match(script, /API_COOKIE_SECURE/u);
    assert.match(script, /docker-compose\.external-db\.yaml/u);
  }
  assert.match(shellScript, /cookie-secure/u);
  assert.match(powershellScript, /CookieSecure/u);

  assert.match(readme, /外部 PostgreSQL/u);
  assert.match(readme, /DATABASE_MODE=external/u);
  assert.match(readme, /docker-compose\.external-db\.yaml/u);
  assert.match(readme, /API_COOKIE_SECURE=false/u);
  assert.match(readme, /API_ALLOWED_ORIGINS=http:\/\/localhost:3200/u);
  assert.match(readme, /127\.0\.0\.1/u);
});

test('docker deploy scripts document the CN accelerator prompt and default proxy URL', () => {
  const shellScript = readRepoFile('scripts', 'shell', 'docker-deploy.sh');
  const powershellScript = readRepoFile('scripts', 'powershell', 'docker-deploy.ps1');

  for (const script of [shellScript, powershellScript]) {
    assert.match(script, /Use CN GitHub plugin download accelerator\?/u);
    assert.match(script, /https:\/\/gh-proxy\.com\//u);
    assert.match(script, /API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL/u);
    assert.match(script, /API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED/u);
    assert.match(script, /DOCKER_DEFAULT_PLATFORM/u);
    assert.match(script, /linux\/amd64/u);
    assert.match(script, /linux\/arm64/u);
    assert.match(script, /docker manifest inspect/u);
    assert.match(script, /docker image inspect/u);
    assert.match(script, /does not publish/u);
    assert.match(script, /Current docker\/.env configuration/u);
    assert.match(script, /Update current docker\/.env configuration\?/u);
    assert.match(script, /Local latest Docker images were found\. Update Docker images\?/u);
    assert.match(script, /Generated API_PROVIDER_SECRET_MASTER_KEY in docker\/.env/u);
    assert.match(script, /change-me-provider-secret-master-key/u);
  }

  assert.match(shellScript, /openssl rand -hex 32/u);
  assert.match(shellScript, /--plugin-github-proxy-url VALUE/u);
  assert.match(shellScript, /--official-plugin-signature-required VALUE/u);
  assert.match(shellScript, /FLOWBASE_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED/u);
  assert.match(powershellScript, /RandomNumberGenerator/u);
  assert.match(powershellScript, /\$PluginGithubProxyUrl/u);
  assert.match(powershellScript, /\$OfficialPluginSignatureRequired/u);
});

test('docker deploy scripts bootstrap a selected portable backup before the API starts', () => {
  const shellScript = readRepoFile('scripts', 'shell', 'docker-deploy.sh');
  const powershellScript = readRepoFile('scripts', 'powershell', 'docker-deploy.ps1');
  const compose = readRepoFile('docker', 'docker-compose.yaml');
  const externalCompose = readRepoFile('docker', 'docker-compose.external-db.yaml');

  for (const script of [shellScript, powershellScript]) {
    assert.match(script, /RESTORE_BACKUP|RestoreBackup/u);
    assert.match(script, /RESTORE_PASSWORD|RestorePassword/u);
    assert.match(script, /bootstrap/u);
    assert.match(script, /API_SYSTEM_BACKUP_PASSWORD/u);
    assert.match(script, /API_PROVIDER_SECRET_MASTER_KEY/u);
    assert.match(script, /deployment\.env/u);
  }
  assert.doesNotMatch(shellScript, /source-master/u);
  assert.doesNotMatch(powershellScript, /source-master/u);
  assert.match(shellScript, /--env-file/u);
  assert.match(powershellScript, /--env-file/u);
  assert.match(shellScript, /recovery-output/u);
  assert.match(powershellScript, /recovery-output/u);
  assert.match(powershellScript, /\[System\.IO\.File\]::Replace/u);
  for (const composeFile of [compose, externalCompose]) {
    assert.match(composeFile, /\.\/recovery:\/recovery:ro/u);
  }
});

test('docker deploy shell bootstrap writes recovery output in a directory before atomically replacing docker env', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-portable-bootstrap-'));
  const tempBin = path.join(tempRoot, 'bin');
  const dockerDir = path.join(tempRoot, 'docker');
  const backupFile = path.join(tempRoot, 'portable.1fb-backup');
  const composeLog = path.join(tempRoot, 'compose.log');
  fs.mkdirSync(tempBin);
  fs.mkdirSync(dockerDir);
  fs.writeFileSync(backupFile, 'portable fixture');
  fs.writeFileSync(
    path.join(dockerDir, '.env.example'),
    [
      'POSTGRES_PASSWORD=fresh-postgres-password',
      'BOOTSTRAP_ROOT_ACCOUNT=root',
      'BOOTSTRAP_ROOT_PASSWORD=fresh-root-password',
      'API_PROVIDER_SECRET_MASTER_KEY=change-me-provider-secret-master-key',
      'DATABASE_MODE=internal',
      'WEB_PORT=3100',
      '',
    ].join('\n'),
  );
  makeExecutable(
    path.join(tempBin, 'docker'),
    `#!/usr/bin/env sh
printf '%s\\n' "$*" >> "${composeLog}"
if [ "$1 $2" = "compose version" ]; then exit 0; fi
if [ "$1" = "info" ]; then
  if [ "$2" = "--format" ]; then printf '%s\\n' 'linux/amd64'; fi
  exit 0
fi
if [ "$1 $2" = "manifest inspect" ]; then
  printf '%s\\n' '{"os":"linux","architecture":"amd64"}'
  exit 0
fi
if [ "$1" = "compose" ] && printf '%s\\n' "$*" | grep -q 'bootstrap'; then
  recovery_output=$(printf '%s\\n' "$*" | sed -n 's/.*--volume \\([^ ]*\\):\\/recovery-output.*/\\1/p')
  if [ -z "$recovery_output" ]; then
    printf '%s\\n' 'bootstrap must receive a writable recovery output directory' >&2
    exit 42
  fi
  sed 's/^API_PROVIDER_SECRET_MASTER_KEY=.*/API_PROVIDER_SECRET_MASTER_KEY=portable-source-master/' "$recovery_output/deployment.env" > "$recovery_output/.deployment.env.tmp"
  mv "$recovery_output/.deployment.env.tmp" "$recovery_output/deployment.env"
fi
exit 0
`,
  );

  const result = spawnSync(
    'sh',
    [
      path.join(repoRoot, 'scripts', 'shell', 'docker-deploy.sh'),
      '--non-interactive',
      '--no-pull',
      '--start',
      '--restore-backup',
      backupFile,
      '--restore-password',
      'password-protected-backup',
    ],
    {
      cwd: tempRoot,
      env: {
        ...process.env,
        PATH: `${tempBin}${path.delimiter}${process.env.PATH || ''}`,
      },
      encoding: 'utf8',
    },
  );

  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(result.stdout, /Portable backup was restored before the API service started\./u);
  assert.doesNotMatch(result.stdout, /portable-source-master/u);
  assert.match(
    fs.readFileSync(path.join(dockerDir, '.env'), 'utf8'),
    /^API_PROVIDER_SECRET_MASTER_KEY=portable-source-master$/mu,
  );
  assert.equal(
    fs.statSync(path.join(dockerDir, 'recovery', 'portable.1fb-backup')).mode & 0o777,
    0o600,
  );
  const calls = fs.readFileSync(composeLog, 'utf8');
  const dependencies = calls.indexOf('compose up -d db plugin-runner');
  const bootstrap = calls.indexOf('bootstrap');
  const finalStartup = calls.lastIndexOf('compose up -d');
  assert.ok(dependencies >= 0, calls);
  assert.ok(bootstrap > dependencies, calls);
  assert.ok(finalStartup > bootstrap, calls);
  assert.match(calls, /--env-file \.recovery-env\./u);
  assert.match(calls, /\/recovery-output\/deployment\.env/u);
  assert.match(calls, /--volume .*\.recovery-output\..*:\/recovery-output/u);
  assert.doesNotMatch(calls, /--volume .*\.env:\/recovery\/deployment\.env/u);
  assert.doesNotMatch(calls, /password-protected-backup/u);
});

const dockerAvailable = spawnSync('docker', ['info'], { stdio: 'ignore' }).status === 0;
const alpineImageAvailable = dockerAvailable
  && spawnSync('docker', ['image', 'inspect', 'alpine:3.20'], { stdio: 'ignore' }).status === 0;

test(
  'real Docker directory bind mount permits recovery configuration atomic replacement',
  { skip: !alpineImageAvailable },
  () => {
    const recoveryOutput = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-recovery-output-'));
    const deploymentEnv = path.join(recoveryOutput, 'deployment.env');
    fs.writeFileSync(deploymentEnv, 'API_PROVIDER_SECRET_MASTER_KEY=old\n', { mode: 0o600 });
    try {
      const singleFileResult = spawnSync(
        'docker',
        [
          'run',
          '--rm',
          '--mount',
          `type=bind,src=${deploymentEnv},dst=/deployment.env`,
          'alpine:3.20',
          'sh',
          '-ec',
          'printf "API_PROVIDER_SECRET_MASTER_KEY=new\\n" > /.deployment.env.tmp && mv /.deployment.env.tmp /deployment.env',
        ],
        { encoding: 'utf8' },
      );
      assert.notEqual(
        singleFileResult.status,
        0,
        'Docker must reject atomic replacement of a single-file bind mount',
      );

      const result = spawnSync(
        'docker',
        [
          'run',
          '--rm',
          '--mount',
          `type=bind,src=${recoveryOutput},dst=/recovery-output`,
          'alpine:3.20',
          'sh',
          '-ec',
          'sed "s/=old$/=portable-source-master/" /recovery-output/deployment.env > /recovery-output/.deployment.env.tmp && mv /recovery-output/.deployment.env.tmp /recovery-output/deployment.env',
        ],
        { encoding: 'utf8' },
      );

      assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
      assert.equal(
        fs.readFileSync(deploymentEnv, 'utf8'),
        'API_PROVIDER_SECRET_MASTER_KEY=portable-source-master\n',
      );
      assert.equal(fs.existsSync(path.join(recoveryOutput, '.deployment.env.tmp')), false);
    } finally {
      fs.rmSync(recoveryOutput, { recursive: true, force: true });
    }
  },
);

test('container image workflow publishes linux amd64 and arm64 manifests', () => {
  const workflow = readRepoFile('.github', 'workflows', 'container-images.yml');

  assert.match(workflow, /docker\/setup-qemu-action@v4/u);
  assert.match(workflow, /^\s+platforms:\s+linux\/amd64,linux\/arm64$/mu);
});

test('container image workflow builds web dist on native runners before publishing', () => {
  const workflow = readRepoFile('.github', 'workflows', 'container-images.yml');

  assert.match(workflow, /select-web:/u);
  assert.match(workflow, /build-web-dist:/u);
  assert.match(workflow, /publish-web:/u);
  assert.match(workflow, /- arch: amd64\n\s+runner: ubuntu-latest/u);
  assert.match(workflow, /- arch: arm64\n\s+runner: ubuntu-24\.04-arm/u);
  assert.match(workflow, /pnpm --dir web install --frozen-lockfile/u);
  assert.match(workflow, /pnpm --dir web --filter @1flowbase\/web build/u);
  assert.match(workflow, /name: web-dist-\$\{\{ matrix\.arch \}\}/u);
  assert.match(workflow, /name: Download web dist artifacts/u);
  assert.match(workflow, /web_dist=\.\/tmp\/web-dist/u);
  assert.match(workflow, /target: runtime-prebuilt/u);
});

test('web Dockerfile keeps local runtime default while exposing prebuilt runtime target', () => {
  const dockerfile = readRepoFile('docker', 'web.Dockerfile');
  const prebuiltStageIndex = dockerfile.indexOf('FROM runtime-base AS runtime-prebuilt');
  const runtimeStageIndex = dockerfile.indexOf('FROM runtime-base AS runtime\n');

  assert.notEqual(prebuiltStageIndex, -1);
  assert.notEqual(runtimeStageIndex, -1);
  assert.ok(runtimeStageIndex > prebuiltStageIndex);
  assert.match(dockerfile, /ARG TARGETARCH\n\nCOPY --from=web_dist \/\$\{TARGETARCH\}\/dist \/usr\/share\/nginx\/html/u);
  assert.match(dockerfile, /FROM runtime-base AS runtime\n\nCOPY --from=builder \/workspace\/web\/app\/dist \/usr\/share\/nginx\/html/u);
});

test('container image workflow scans temporary image tags before official promotion', () => {
  const workflow = readRepoFile('.github', 'workflows', 'container-images.yml');

  assert.match(workflow, /permissions:\n\s+contents: read/u);
  assert.match(
    workflow,
    /publish-web:\n\s+needs:\n\s+- select-web\n\s+- build-web-dist\n\s+if: needs\.select-web\.outputs\.enabled == 'true'\n\s+runs-on: ubuntu-latest\n\s+permissions:\n\s+contents: read\n\s+packages: write/u,
  );
  assert.match(workflow, /publish-api-server:/u);
  assert.match(workflow, /publish-plugin-runner:/u);

  assert.match(workflow, /scan_tag=scan-\$\{\{ github\.run_id \}\}-\$\{\{ github\.run_attempt \}\}-\$\{\{ github\.sha \}\}/u);
  assert.match(workflow, /echo "scan_image_ref=ghcr\.io\/\$\{\{ github\.repository_owner \}\}\/1flowbase-web:\$scan_tag"/u);
  assert.match(workflow, /name: Build and push prebuilt web scan candidate/u);
  assert.match(workflow, /\$\{\{ steps\.image_refs\.outputs\.scan_image_ref \}\}/u);
  assert.doesNotMatch(workflow, /Build and push[\s\S]*?ghcr\.io\/\$\{\{ github\.repository_owner \}\}\/1flowbase-web:\$\{\{ needs\.select-web\.outputs\.image_tag \}\}/u);
  assert.doesNotMatch(workflow, /Build and push[\s\S]*?ghcr\.io\/\$\{\{ github\.repository_owner \}\}\/1flowbase-web:latest/u);

  assert.match(workflow, /aquasecurity\/trivy-action@ed142fd0673e97e23eac54620cfb913e5ce36c25/u);
  assert.match(workflow, /version: v0\.70\.0/u);
  assert.match(workflow, /name: Generate HIGH Trivy warning report/u);
  assert.match(workflow, /severity: HIGH/u);
  assert.match(workflow, /exit-code: "0"/u);
  assert.match(workflow, /name: Enforce CRITICAL Trivy release gate/u);
  assert.match(workflow, /severity: CRITICAL/u);
  assert.match(workflow, /exit-code: "0"/u);
  assert.match(workflow, /output: tmp\/test-governance\/trivy-web-high\.json/u);
  assert.match(workflow, /output: tmp\/test-governance\/trivy-web-critical\.json/u);

  assert.match(workflow, /name: Promote scanned image to official tags/u);
  assert.match(
    workflow,
    /if: github\.event_name != 'workflow_dispatch' \|\| inputs\.promote_official_tags == true \|\| inputs\.promote_official_tags == 'true'/u,
  );
  assert.equal(
    [
      ...workflow.matchAll(
        /if: github\.event_name != 'workflow_dispatch' \|\| inputs\.promote_official_tags == true \|\| inputs\.promote_official_tags == 'true'/gu,
      ),
    ].length,
    3,
  );
  assert.match(workflow, /docker buildx imagetools create/u);
  assert.match(workflow, /--tag "\$\{\{ steps\.image_refs\.outputs\.version_image_ref \}\}"/u);
  assert.match(workflow, /--tag "\$\{\{ steps\.image_refs\.outputs\.latest_image_ref \}\}"/u);
  assert.match(workflow, /"\$\{\{ steps\.image_refs\.outputs\.scan_image_ref \}\}"/u);
  assert.match(workflow, /name: test-governance-trivy-web/u);
  assert.match(workflow, /path: tmp\/test-governance\/trivy-web-\*\.json/u);
});

test('container image workflow publishes a GitHub release for the latest API version', () => {
  const workflow = readRepoFile('.github', 'workflows', 'container-images.yml');

  assert.match(workflow, /publish-release:\n\s+if: \$\{\{ always\(\) && github\.event_name == 'push' && github\.ref == 'refs\/heads\/latest'/u);
  assert.match(workflow, /permissions:\n\s+contents: write/u);
  assert.match(workflow, /RELEASE_TAG: \$\{\{ needs\.select-api-server\.outputs\.image_tag \}\}/u);
  assert.match(workflow, /gh release view "\$RELEASE_TAG"/u);
  assert.match(workflow, /cat > release-notes\.md <<EOF/u);
  assert.match(workflow, /## Installation or Upgrade/u);
  assert.match(workflow, /scripts\/shell\/docker-deploy\.sh/u);
  assert.match(workflow, /scripts\/powershell\/docker-deploy\.ps1/u);
  assert.match(workflow, /Windows CMD/u);
  assert.match(workflow, /NoProfile -ExecutionPolicy Bypass/u);
  assert.match(workflow, /## Docker Images/u);
  assert.match(workflow, /ghcr\.io\/\$\{\{ github\.repository_owner \}\}\/1flowbase-api-server:\$RELEASE_TAG/u);
  assert.match(workflow, /## Traceability/u);
  assert.match(workflow, /Target commit/u);
  assert.match(workflow, /Workflow run/u);
  assert.match(workflow, /gh release create "\$RELEASE_TAG"/u);
  assert.match(workflow, /--notes-file release-notes\.md/u);
  assert.match(workflow, /--generate-notes/u);
});

test('container image workflow records a CD quality gate artifact for Trivy reports', () => {
  const workflow = readRepoFile('.github', 'workflows', 'container-images.yml');

  assert.match(
    workflow,
    /report:\n\s+if: \$\{\{ always\(\) \}\}\n\s+needs:\n\s+- publish-web\n\s+- publish-api-server\n\s+- publish-plugin-runner\n\s+runs-on: ubuntu-latest\n\s+permissions:\n\s+contents: read\n\s+actions: read\n\s+issues: write/u,
  );
  assert.match(workflow, /pattern: test-governance-trivy-\*/u);
  assert.match(workflow, /path: tmp\/test-governance/u);
  assert.match(workflow, /merge-multiple: true/u);
  assert.match(workflow, /uses: \.\/\.github\/actions\/quality-gate/u);
  assert.match(workflow, /scope: container-images/u);
  assert.match(workflow, /report_type: cd/u);
  assert.match(workflow, /environment: container-images/u);
  assert.match(workflow, /publish_issue: "false"/u);
  assert.match(workflow, /start_postgres: "false"/u);
  assert.match(workflow, /github_token: \$\{\{ secrets\.GITHUB_TOKEN \}\}/u);
  assert.match(workflow, /name: test-governance-container-images/u);
  assert.match(workflow, /path: tmp\/test-governance/u);
});

test('docker deploy shell script stops before pull when the image tag lacks the detected platform', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-docker-deploy-'));
  const tempBin = path.join(tempRoot, 'bin');
  const dockerDir = path.join(tempRoot, 'docker');
  fs.mkdirSync(tempBin);
  fs.mkdirSync(dockerDir);
  fs.writeFileSync(
    path.join(dockerDir, '.env.example'),
    [
      'FLOWBASE_WEB_VERSION=latest',
      'FLOWBASE_API_SERVER_VERSION=latest',
      'FLOWBASE_PLUGIN_RUNNER_VERSION=latest',
      'POSTGRES_PASSWORD=change-me',
      'API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=',
      '',
    ].join('\n'),
  );
  makeExecutable(
    path.join(tempBin, 'docker'),
    `#!/usr/bin/env sh
if [ "$1 $2" = "compose version" ]; then exit 0; fi
if [ "$1" = "info" ]; then
  if [ "$2" = "--format" ]; then
    printf '%s\\n' 'linux/aarch64'
  fi
  exit 0
fi
if [ "$1 $2" = "manifest inspect" ]; then
  cat <<'EOF'
{
  "schemaVersion": 2,
  "manifests": [
    { "platform": { "architecture": "amd64", "os": "linux" } }
  ]
}
EOF
  exit 0
fi
if [ "$1 $2" = "compose pull" ]; then
  printf '%s\\n' 'compose pull should not run'
  exit 42
fi
exit 0
`,
  );

  const result = spawnSync(
    'sh',
    [
      path.join(repoRoot, 'scripts', 'shell', 'docker-deploy.sh'),
      '--non-interactive',
      '--pull',
    ],
    {
      cwd: tempRoot,
      env: {
        ...process.env,
        PATH: `${tempBin}${path.delimiter}${process.env.PATH || ''}`,
      },
      encoding: 'utf8',
    },
  );

  assert.notEqual(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(result.stderr, /does not publish linux\/arm64/u);
  assert.doesNotMatch(result.stdout, /compose pull should not run/u);
});
