const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');
const { spawn, spawnSync } = require('node:child_process');

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

const shAvailable = process.platform !== 'win32'
  && spawnSync('sh', ['-c', 'exit 0'], { stdio: 'ignore' }).status === 0;
const scriptTtyAvailable = shAvailable
  && spawnSync('script', ['-V'], { stdio: 'ignore' }).status === 0;
const powershellBin = ['pwsh', 'powershell'].find(
  (bin) => spawnSync(bin, ['-NoProfile', '-Command', 'exit 0'], { stdio: 'ignore' }).status === 0,
);
const linuxDockerAvailable = process.platform === 'linux'
  && spawnSync('docker', ['info'], { stdio: 'ignore' }).status === 0;
const bindMountProbeImage = linuxDockerAvailable
  ? ['ghcr.io/taichuy/1flowbase-api-server:latest', 'alpine:3.20'].find(
      (img) => spawnSync('docker', ['image', 'inspect', img], { stdio: 'ignore' }).status === 0,
    )
  : null;

const writableApiMountTargets = [
  '/app/api/storage',
  '/app/api/plugins/packages',
  '/app/api/plugins/installed',
  '/app/api/plugins/host-extension/dropins',
];

test('AC-001: all Docker Compose files align writable API bind mounts via a 0:0 api-init service before starting api', () => {
  for (const composeName of [
    'docker-compose.yaml',
    'docker-compose.dev.yaml',
    'docker-compose.external-db.yaml',
  ]) {
    const compose = readRepoFile('docker', composeName);
    assert.match(compose, /^\s{2}api-init:\r?$/mu, `${composeName} must declare api-init`);
    assert.match(compose, /^\s{4}user:\s*["']?0:0["']?\r?$/mu, `${composeName} api-init must run as 0:0`);
    assert.match(compose, /^\s{4}restart:\s*["']?no["']?\r?$/mu, `${composeName} api-init must not restart in a loop`);
    assert.match(
      compose,
      /api-init:\r?\n\s+condition:\s*service_completed_successfully/u,
      `${composeName} api must wait for api-init completion`,
    );
    assert.match(compose, /\/home\/flowbase/u, `${composeName} api-init must resolve flowbase uid/gid from image`);
    assert.match(compose, /chown -h/u, `${composeName} api-init must avoid following symlinks`);
    for (const mountTarget of writableApiMountTargets) {
      assert.ok(
        compose.includes(mountTarget),
        `${composeName} must include writable mount ${mountTarget}`,
      );
    }
  }
});

test(
  'AC-002: fresh non-interactive shell install generates a random POSTGRES_PASSWORD and accepts --allowed-origins',
  { skip: !shAvailable },
  () => {
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-fresh-env-'));
    const tempBin = path.join(tempRoot, 'bin');
    const dockerDir = path.join(tempRoot, 'docker');
    try {
      fs.mkdirSync(tempBin);
      fs.mkdirSync(dockerDir);
      fs.copyFileSync(path.join(repoRoot, 'docker', '.env.example'), path.join(dockerDir, '.env.example'));
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
          '--allowed-origins',
          'https://ai.example.com,http://localhost:3100',
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
      const envFile = fs.readFileSync(path.join(dockerDir, '.env'), 'utf8');
      assert.match(result.stdout, /Generated POSTGRES_PASSWORD in docker\/\.env\./u);
      assert.doesNotMatch(envFile, /^POSTGRES_PASSWORD=change-me-db-password$/mu);
      assert.match(envFile, /^POSTGRES_PASSWORD=[0-9a-f]{48}$/mu);
      assert.match(
        envFile,
        /^API_ALLOWED_ORIGINS=https:\/\/ai\.example\.com,http:\/\/localhost:3100$/mu,
      );
      assert.match(envFile, /^API_COOKIE_SECURE=true$/mu);
    } finally {
      fs.rmSync(tempRoot, { recursive: true, force: true });
    }
  },
);

test(
  'AC-002b: fresh interactive shell install defaults Pull and Start prompts to yes on bare Enter',
  { skip: !scriptTtyAvailable },
  () => {
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-fresh-tty-'));
    const tempBin = path.join(tempRoot, 'bin');
    const dockerDir = path.join(tempRoot, 'docker');
    const callsFile = path.join(tempRoot, 'docker-calls.log');
    try {
      fs.mkdirSync(tempBin);
      fs.mkdirSync(dockerDir);
      fs.copyFileSync(path.join(repoRoot, 'docker', '.env.example'), path.join(dockerDir, '.env.example'));
      makeExecutable(
        path.join(tempBin, 'docker'),
        `#!/usr/bin/env sh
printf '%s\\n' "$*" >> ${shellQuote(callsFile)}
if [ "$1 $2" = "compose version" ]; then exit 0; fi
if [ "$1 $2" = "image inspect" ]; then exit 1; fi
if [ "$1" = "info" ]; then
  if [ "$2" = "--format" ]; then printf '%s\\n' 'linux/amd64'; fi
  exit 0
fi
if [ "$1 $2" = "manifest inspect" ]; then
  printf '%s\\n' '{"schemaVersion":2,"manifests":[{"platform":{"architecture":"amd64","os":"linux"}}]}'
  exit 0
fi
exit 0
`,
      );

      const result = spawnSync(
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
          input: '\n'.repeat(16),
          encoding: 'utf8',
        },
      );

      assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
      const calls = fs.readFileSync(callsFile, 'utf8');
      assert.match(calls, /^compose pull$/mu);
      assert.match(calls, /^compose up -d --remove-orphans$/mu);
    } finally {
      fs.rmSync(tempRoot, { recursive: true, force: true });
    }
  },
);

test(
  'AC-003: docker-deploy shell script fails with container logs when api enters a restart crash-loop after compose up',
  { skip: !shAvailable },
  () => {
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-crash-detect-'));
    const tempBin = path.join(tempRoot, 'bin');
    const dockerDir = path.join(tempRoot, 'docker');
    try {
      fs.mkdirSync(tempBin);
      fs.mkdirSync(dockerDir);
      fs.copyFileSync(path.join(repoRoot, 'docker', '.env.example'), path.join(dockerDir, '.env.example'));
      makeExecutable(
        path.join(tempBin, 'docker'),
        `#!/usr/bin/env sh
if [ "$1 $2" = "compose version" ]; then exit 0; fi
if [ "$1" = "info" ]; then
  if [ "$2" = "--format" ]; then printf '%s\\n' 'linux/amd64'; fi
  exit 0
fi
if [ "$1 $2" = "manifest inspect" ]; then
  printf '%s\\n' '{"schemaVersion":2,"manifests":[{"platform":{"architecture":"amd64","os":"linux"}}]}'
  exit 0
fi
if [ "$1 $2 $3 $4" = "compose ps -q api" ]; then
  printf '%s\\n' 'api-container-crash-123'
  exit 0
fi
if [ "$1" = "inspect" ]; then
  printf '%s\\n' 'restarting:1:1'
  exit 0
fi
if [ "$1" = "logs" ]; then
  printf '%s\\n' 'Error: failed to create installation artifact root /app/api/plugins/installed/1flowbase' >&2
  printf '%s\\n' 'Caused by: Permission denied (os error 13)' >&2
  exit 0
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
        ],
        {
          cwd: tempRoot,
          env: {
            ...process.env,
            FLOWBASE_STARTUP_WAIT_SECONDS: '6',
            PATH: `${tempBin}${path.delimiter}${process.env.PATH || ''}`,
          },
          encoding: 'utf8',
        },
      );

      assert.notEqual(result.status, 0, 'deploy script must exit non-zero on API crash-loop');
      assert.match(`${result.stdout}\n${result.stderr}`, /Permission denied \(os error 13\)/u);
    } finally {
      fs.rmSync(tempRoot, { recursive: true, force: true });
    }
  },
);

test(
  'AC-003b: docker-deploy shell script succeeds once running api container passes HTTP /health readiness',
  { skip: !shAvailable },
  async () => {
    const server = http.createServer((req, res) => {
      if (req.url === '/health') {
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end('{"status":"ok"}');
        return;
      }
      res.writeHead(404);
      res.end();
    });
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    const { port } = server.address();

    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-ready-ok-'));
    const tempBin = path.join(tempRoot, 'bin');
    const dockerDir = path.join(tempRoot, 'docker');
    try {
      fs.mkdirSync(tempBin);
      fs.mkdirSync(dockerDir);
      fs.copyFileSync(path.join(repoRoot, 'docker', '.env.example'), path.join(dockerDir, '.env.example'));
      makeExecutable(
        path.join(tempBin, 'docker'),
        `#!/usr/bin/env sh
if [ "$1 $2" = "compose version" ]; then exit 0; fi
if [ "$1" = "info" ]; then
  if [ "$2" = "--format" ]; then printf '%s\\n' 'linux/amd64'; fi
  exit 0
fi
if [ "$1 $2" = "manifest inspect" ]; then
  printf '%s\\n' '{"schemaVersion":2,"manifests":[{"platform":{"architecture":"amd64","os":"linux"}}]}'
  exit 0
fi
if [ "$1 $2 $3 $4" = "compose ps -q api" ]; then
  printf '%s\\n' 'api-container-ok-123'
  exit 0
fi
if [ "$1" = "inspect" ]; then
  printf '%s\\n' 'running:0:0'
  exit 0
fi
exit 0
`,
      );

      const result = await new Promise((resolve, reject) => {
        const child = spawn(
          'sh',
          [
            path.join(repoRoot, 'scripts', 'shell', 'docker-deploy.sh'),
            '--non-interactive',
            '--no-pull',
            '--start',
            '--web-port',
            String(port),
          ],
          {
            cwd: tempRoot,
            env: {
              ...process.env,
              FLOWBASE_STARTUP_WAIT_SECONDS: '10',
              PATH: `${tempBin}${path.delimiter}${process.env.PATH || ''}`,
            },
          },
        );
        let stdout = '';
        let stderr = '';
        child.stdout.setEncoding('utf8');
        child.stderr.setEncoding('utf8');
        child.stdout.on('data', (chunk) => { stdout += chunk; });
        child.stderr.on('data', (chunk) => { stderr += chunk; });
        child.on('error', reject);
        child.on('close', (status) => resolve({ status, stdout, stderr }));
      });

      assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
      assert.match(result.stdout, new RegExp(`Verified 1flowbase health at http://127\\.0\\.0\\.1:${port}/health\\.`, 'u'));
    } finally {
      await new Promise((resolve) => server.close(resolve));
      fs.rmSync(tempRoot, { recursive: true, force: true });
    }
  },
);

test(
  'AC-002c: PowerShell deploy script executes fresh password generation, AllowedOrigins, and startup crash-loop detection',
  { skip: !powershellBin },
  () => {
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-ps-deploy-'));
    const tempBin = path.join(tempRoot, 'bin');
    const dockerDir = path.join(tempRoot, 'docker');
    try {
      fs.mkdirSync(tempBin);
      fs.mkdirSync(dockerDir);
      fs.copyFileSync(path.join(repoRoot, 'docker', '.env.example'), path.join(dockerDir, '.env.example'));
      if (process.platform === 'win32') {
        fs.writeFileSync(
          path.join(tempBin, 'docker.cmd'),
          [
            '@echo off',
            'if "%1 %2"=="compose version" exit /b 0',
            'if "%1"=="info" (',
            '  if "%2"=="--format" echo linux/amd64',
            '  exit /b 0',
            ')',
            'if "%1 %2"=="manifest inspect" (',
            '  echo {"schemaVersion":2,"manifests":[{"platform":{"architecture":"amd64","os":"linux"}}]}',
            '  exit /b 0',
            ')',
            'if "%1 %2 %3 %4"=="compose ps -q api" (',
            '  echo api-crash-ps-1',
            '  exit /b 0',
            ')',
            'if "%1"=="inspect" (',
            '  echo restarting:1:1',
            '  exit /b 0',
            ')',
            'if "%1"=="logs" (',
            '  echo Permission denied ^(os error 13^)',
            '  exit /b 0',
            ')',
            'exit /b 0',
            '',
          ].join('\r\n'),
        );
      } else {
        makeExecutable(
          path.join(tempBin, 'docker'),
          `#!/usr/bin/env sh
if [ "$1 $2" = "compose version" ]; then exit 0; fi
if [ "$1" = "info" ]; then
  if [ "$2" = "--format" ]; then printf '%s\\n' 'linux/amd64'; fi
  exit 0
fi
if [ "$1 $2" = "manifest inspect" ]; then
  printf '%s\\n' '{"schemaVersion":2,"manifests":[{"platform":{"architecture":"amd64","os":"linux"}}]}'
  exit 0
fi
if [ "$1 $2 $3 $4" = "compose ps -q api" ]; then
  printf '%s\\n' 'api-crash-ps-1'
  exit 0
fi
if [ "$1" = "inspect" ]; then
  printf '%s\\n' 'restarting:1:1'
  exit 0
fi
if [ "$1" = "logs" ]; then
  printf '%s\\n' 'Permission denied (os error 13)'
  exit 0
fi
exit 0
`,
        );
      }

      const prepResult = spawnSync(
        powershellBin,
        [
          '-NoProfile',
          '-ExecutionPolicy',
          'Bypass',
          '-File',
          path.join(repoRoot, 'scripts', 'powershell', 'docker-deploy.ps1'),
          '-NonInteractive',
          '-NoPull',
          '-NoStart',
          '-AllowedOrigins',
          'https://ai.example.com,http://localhost:3100',
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
      assert.equal(prepResult.status, 0, `${prepResult.stdout}\n${prepResult.stderr}`);
      const envFile = fs.readFileSync(path.join(dockerDir, '.env'), 'utf8');
      assert.match(prepResult.stdout, /Generated POSTGRES_PASSWORD in docker\/\.env\./u);
      assert.match(envFile, /^POSTGRES_PASSWORD=[0-9a-f]{48}\r?$/mu);
      assert.match(envFile, /^API_ALLOWED_ORIGINS=https:\/\/ai\.example\.com,http:\/\/localhost:3100\r?$/mu);
      assert.match(envFile, /^API_COOKIE_SECURE=true\r?$/mu);

      const crashResult = spawnSync(
        powershellBin,
        [
          '-NoProfile',
          '-ExecutionPolicy',
          'Bypass',
          '-File',
          path.join(repoRoot, 'scripts', 'powershell', 'docker-deploy.ps1'),
          '-NonInteractive',
          '-NoPull',
          '-Start',
        ],
        {
          cwd: tempRoot,
          env: {
            ...process.env,
            FLOWBASE_STARTUP_WAIT_SECONDS: '5',
            PATH: `${tempBin}${path.delimiter}${process.env.PATH || ''}`,
          },
          encoding: 'utf8',
        },
      );
      assert.notEqual(crashResult.status, 0, 'PowerShell deploy script must fail on API crash-loop');
      assert.match(`${crashResult.stdout}\n${crashResult.stderr}`, /Permission denied \(os error 13\)/u);
    } finally {
      fs.rmSync(tempRoot, { recursive: true, force: true });
    }
  },
);

test(
  'AC-004: real Linux Docker bind mount owned by root/non-1000 becomes writable by uid 1000 after 0:0 api-init without following symlinks',
  { skip: !bindMountProbeImage },
  () => {
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-bind-uid-'));
    try {
      const storageHost = path.join(tempRoot, 'storage');
      const installedHost = path.join(tempRoot, 'installed');
      const outsideFile = path.join(tempRoot, 'outside.txt');
      fs.mkdirSync(storageHost, { mode: 0o755 });
      fs.mkdirSync(installedHost, { mode: 0o755 });
      fs.writeFileSync(outsideFile, 'safe\n');

      // Stage root-owned (0:0, 0755) directory and a symlink to outsideFile to simulate non-1000 fresh install + symlink hazard.
      const setupRoot = spawnSync(
        'docker',
        [
          'run',
          '--rm',
          '-u',
          '0:0',
          '--entrypoint',
          '/bin/sh',
          '-v',
          `${tempRoot}:/host`,
          bindMountProbeImage,
          '-ceu',
          'chown 0:0 /host/storage /host/installed && chmod 0755 /host/storage /host/installed && chown 1001:1001 /host/outside.txt && ln -s /host/outside.txt /host/storage/escape-link',
        ],
        { encoding: 'utf8' },
      );
      assert.equal(setupRoot.status, 0, `${setupRoot.stdout}\n${setupRoot.stderr}`);

      // Before api-init, uid 1000:1000 MUST fail with Permission denied.
      const beforeInit = spawnSync(
        'docker',
        [
          'run',
          '--rm',
          '-u',
          '1000:1000',
          '--entrypoint',
          '/bin/sh',
          '-v',
          `${installedHost}:/app/api/plugins/installed`,
          bindMountProbeImage,
          '-ceu',
          'mkdir -p /app/api/plugins/installed/1flowbase/2.0.installing-probe',
        ],
        { encoding: 'utf8' },
      );
      assert.notEqual(beforeInit.status, 0, 'unaligned 0:0 0755 mount must reject uid 1000 writes');

      // Run the exact api-init ownership alignment as 0:0 targeting 1000:1000.
      const runInit = spawnSync(
        'docker',
        [
          'run',
          '--rm',
          '-u',
          '0:0',
          '--entrypoint',
          '/bin/sh',
          '-v',
          `${storageHost}:/app/api/storage`,
          '-v',
          `${installedHost}:/app/api/plugins/installed`,
          bindMountProbeImage,
          '-ceu',
          'owner=1000:1000; uid=1000; gid=1000; for directory in /app/api/storage /app/api/plugins/installed; do mkdir -p "$directory"; find "$directory" \\( ! -user "$uid" -o ! -group "$gid" \\) -exec chown -h "$owner" {} +; done',
        ],
        { encoding: 'utf8' },
      );
      assert.equal(runInit.status, 0, `${runInit.stdout}\n${runInit.stderr}`);

      // After api-init, uid 1000:1000 writes cleanly AND outside.txt owner (1001:1001) was NOT clobbered via symlink!
      const afterInit = spawnSync(
        'docker',
        [
          'run',
          '--rm',
          '-u',
          '1000:1000',
          '--entrypoint',
          '/bin/sh',
          '-v',
          `${storageHost}:/app/api/storage`,
          '-v',
          `${installedHost}:/app/api/plugins/installed`,
          bindMountProbeImage,
          '-ceu',
          'mkdir -p /app/api/storage/extension-center/mcp /app/api/plugins/installed/1flowbase/2.0.installing-probe',
        ],
        { encoding: 'utf8' },
      );
      assert.equal(afterInit.status, 0, `${afterInit.stdout}\n${afterInit.stderr}`);

      const checkSymlink = spawnSync(
        'docker',
        [
          'run',
          '--rm',
          '-u',
          '0:0',
          '--entrypoint',
          '/bin/sh',
          '-v',
          `${tempRoot}:/host`,
          bindMountProbeImage,
          '-ceu',
          'owner="$(stat -c "%u:%g" /host/outside.txt)" && rm -rf /host/* && printf "%s" "$owner"',
        ],
        { encoding: 'utf8' },
      );
      assert.equal(checkSymlink.status, 0, `${checkSymlink.stdout}\n${checkSymlink.stderr}`);
      assert.equal(checkSymlink.stdout.trim(), '1001:1001', 'chown -h must not dereference symlinks outside mount');
    } finally {
      fs.rmSync(tempRoot, { recursive: true, force: true });
    }
  },
);
