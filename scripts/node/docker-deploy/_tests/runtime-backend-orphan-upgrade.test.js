const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');

function shellQuote(value) {
  return `'${String(value).replace(/'/gu, "'\\''")}'`;
}

test('docker deploy upgrade removes the legacy plugin-runner orphan', () => {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-runtime-orphan-'));
  const tempBin = path.join(tempRoot, 'bin');
  const dockerDir = path.join(tempRoot, 'docker');
  const callsFile = path.join(tempRoot, 'docker-calls.log');
  const orphanFile = path.join(tempRoot, '1flowbase-plugin-runner-1');
  fs.mkdirSync(tempBin);
  fs.mkdirSync(dockerDir);
  fs.writeFileSync(orphanFile, 'legacy runtime backend\n');
  fs.writeFileSync(
    path.join(dockerDir, '.env.example'),
    [
      'FLOWBASE_WEB_VERSION=latest',
      'FLOWBASE_API_SERVER_VERSION=latest',
      'DATABASE_MODE=internal',
      'POSTGRES_DB=1flowbase',
      'POSTGRES_USER=postgres',
      'POSTGRES_PASSWORD=example-password',
      'BOOTSTRAP_ROOT_PASSWORD=example-root-password',
      'API_PROVIDER_SECRET_MASTER_KEY=example-secret',
      'API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL=',
      '',
    ].join('\n'),
  );
  fs.writeFileSync(path.join(dockerDir, '.env'), fs.readFileSync(path.join(dockerDir, '.env.example')));
  fs.writeFileSync(
    path.join(tempBin, 'docker'),
    `#!/usr/bin/env sh
printf '%s\\n' "$*" >> ${shellQuote(callsFile)}
if [ "$1 $2" = "compose version" ]; then exit 0; fi
if [ "$1" = "info" ]; then
  if [ "$2" = "--format" ]; then printf '%s\\n' 'linux/amd64'; fi
  exit 0
fi
if [ "$1 $2" = "manifest inspect" ]; then
  printf '%s\\n' '{"schemaVersion":2,"manifests":[{"platform":{"architecture":"amd64","os":"linux"}}]}'
  exit 0
fi
case " $* " in
  *" compose up -d --remove-orphans "*) rm -f ${shellQuote(orphanFile)} ;;
esac
exit 0
`,
    { mode: 0o755 },
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
        PATH: `${tempBin}${path.delimiter}${process.env.PATH || ''}`,
      },
      encoding: 'utf8',
    },
  );

  const calls = fs.readFileSync(callsFile, 'utf8');
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(calls, /^compose up -d --remove-orphans$/mu);
  assert.equal(fs.existsSync(orphanFile), false, 'legacy Runtime Backend orphan must be removed');
});

test('docker deploy start paths and delayed commands always remove legacy orphans', () => {
  const shellScript = fs.readFileSync(
    path.join(repoRoot, 'scripts', 'shell', 'docker-deploy.sh'),
    'utf8',
  );
  const powershellScript = fs.readFileSync(
    path.join(repoRoot, 'scripts', 'powershell', 'docker-deploy.ps1'),
    'utf8',
  );

  assert.match(shellScript, /compose -f docker-compose\.external-db\.yaml up -d --remove-orphans/u);
  assert.match(shellScript, /compose up -d --remove-orphans/u);
  assert.doesNotMatch(shellScript, /docker compose(?: -f docker-compose\.external-db\.yaml)? up -d"/u);

  assert.match(
    powershellScript,
    /Invoke-ComposeCommand @\("-f", "docker-compose\.external-db\.yaml", "up", "-d", "--remove-orphans"\)/u,
  );
  assert.match(powershellScript, /Invoke-ComposeCommand @\("up", "-d", "--remove-orphans"\)/u);
  assert.doesNotMatch(
    powershellScript,
    /docker compose(?: -f docker-compose\.external-db\.yaml)? up -d"/u,
  );
});
