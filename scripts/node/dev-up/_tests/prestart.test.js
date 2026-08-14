const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('path');

const {
  buildServiceEnv,
  ensureServiceEnvFile,
  getServiceDefinitions,
  getServicePrestartCommands,
  runServicePrestartCommands,
} = require('../core.js');

test('AC-001 runs the local frontstage executable upgrade before resetting the api root password', () => {
  const tempRepoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-prestart-'));
  const apiServerDir = path.join(tempRepoRoot, 'api', 'apps', 'api-server');
  const envExamplePath = path.join(apiServerDir, '.env.example');

  fs.mkdirSync(apiServerDir, { recursive: true });
  fs.writeFileSync(
    envExamplePath,
    ['API_ENV=development', 'API_DATABASE_URL=postgres://from-example'].join('\n')
  );

  const services = getServiceDefinitions(tempRepoRoot);
  const apiService = services['api-server'];
  ensureServiceEnvFile(apiService);

  const commands = getServicePrestartCommands(apiService, {});

  assert.deepEqual(
    commands.map((command) => ({
      description: command.description,
      command: command.command,
      args: command.args,
      cwd: command.cwd,
    })),
    [
      {
        description: 'api-server development frontstage executable upgrade',
        command: 'cargo',
        args: ['run', '-p', 'api-server', '--bin', 'frontstage_executable_upgrade'],
        cwd: path.join(tempRepoRoot, 'api'),
      },
      {
        description: 'api-server development root password reset',
        command: 'cargo',
        args: ['run', '-p', 'api-server', '--bin', 'reset_root_password'],
        cwd: path.join(tempRepoRoot, 'api'),
      },
    ]
  );
  assert.equal(commands[0].env.API_ENV, 'development');
  assert.equal(
    commands[0].env.API_FRONTSTAGE_EXECUTABLE_COMPILER_ROOT,
    path.join(tempRepoRoot, 'web')
  );
  assert.equal(commands[0].env.API_FRONTSTAGE_EXECUTABLE_NODE_PATH, process.execPath);
});

test('getServicePrestartCommands checks frontend dependencies with visible pnpm prompts', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const services = getServiceDefinitions(repoRoot);
  const commands = getServicePrestartCommands(services.web, { CI: 'false' });

  assert.deepEqual(
    commands.map((command) => ({
      description: command.description,
      command: command.command,
      args: command.args,
      cwd: command.cwd,
      captureOutput: command.captureOutput,
      ci: command.env.CI,
    })),
    [
      {
        description:
          'frontend dependency check (pnpm will prompt in the terminal if a clean reinstall is required)',
        command: 'pnpm',
        args: ['install'],
        cwd: path.join(repoRoot, 'web'),
        captureOutput: false,
        ci: 'false',
      },
    ]
  );
});

test('getServicePrestartCommands skips api root reset in production mode', () => {
  const tempRepoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-prod-'));
  const apiServerDir = path.join(tempRepoRoot, 'api', 'apps', 'api-server');
  const envExamplePath = path.join(apiServerDir, '.env.example');

  fs.mkdirSync(apiServerDir, { recursive: true });
  fs.writeFileSync(
    envExamplePath,
    ['API_ENV=production', 'API_DATABASE_URL=postgres://from-example'].join('\n')
  );

  const services = getServiceDefinitions(tempRepoRoot);
  const apiService = services['api-server'];
  ensureServiceEnvFile(apiService);

  assert.deepEqual(getServicePrestartCommands(apiService, {}), []);
});

test('AC-003 blocks later api pre-start steps when the frontstage upgrade fails', () => {
  const tempRepoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-recover-'));
  const apiServerDir = path.join(tempRepoRoot, 'api', 'apps', 'api-server');
  const dockerDir = path.join(tempRepoRoot, 'docker');

  fs.mkdirSync(apiServerDir, { recursive: true });
  fs.mkdirSync(dockerDir, { recursive: true });

  fs.writeFileSync(
    path.join(apiServerDir, '.env.example'),
    [
      'API_ENV=development',
      'API_DATABASE_URL=postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase',
      'BOOTSTRAP_WORKSPACE_NAME=1flowbase',
      'BOOTSTRAP_ROOT_ACCOUNT=root',
      'BOOTSTRAP_ROOT_EMAIL=root@example.com',
      'BOOTSTRAP_ROOT_PASSWORD=change-me',
    ].join('\n')
  );
  fs.writeFileSync(path.join(dockerDir, 'middleware.env'), 'POSTGRES_PORT=35432\n');

  const services = getServiceDefinitions(tempRepoRoot);
  const apiService = services['api-server'];
  ensureServiceEnvFile(apiService);

  const commandCalls = [];
  const composeCalls = [];
  const originalStderrWrite = process.stderr.write;
  let stderrOutput = '';

  process.stderr.write = (chunk, encoding, callback) => {
    stderrOutput += String(chunk);
    if (typeof encoding === 'function') {
      encoding();
    }
    if (typeof callback === 'function') {
      callback();
    }
    return true;
  };

  try {
    assert.throws(
      () =>
        runServicePrestartCommands(apiService, {
          logImpl() {},
          runCommandImpl(command, args, options) {
            commandCalls.push({ command, args, options });
            return {
              status: 1,
              stdout: '',
              stderr: 'Error: migration 20260412183000 was previously applied but has been modified\n',
            };
          },
          runMiddlewareComposeImpl(repoRoot, args) {
            composeCalls.push({ repoRoot, args });
            return {
              status: 0,
              stdout: '',
              stderr: '',
            };
          }
        }),
      /api-server development frontstage executable upgrade failed with exit code 1/u
    );
  } finally {
    process.stderr.write = originalStderrWrite;
  }

  assert.equal(commandCalls.length, 1);
  assert.equal(commandCalls[0].args.at(-1), 'frontstage_executable_upgrade');
  assert.equal(commandCalls[0].options.captureOutput, true);
  assert.deepEqual(composeCalls, []);
  assert.equal(
    stderrOutput.match(/previously applied but has been modified/gu)?.length,
    1
  );
});

test('AC-001 repairs the known local migration checksum drift without rebuilding postgres', () => {
  const tempRepoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-repair-'));
  const apiServerDir = path.join(tempRepoRoot, 'api', 'apps', 'api-server');
  const migrationDir = path.join(
    tempRepoRoot,
    'api',
    'crates',
    'storage-durable',
    'postgres',
    'migrations'
  );
  const dockerDir = path.join(tempRepoRoot, 'docker');

  fs.mkdirSync(apiServerDir, { recursive: true });
  fs.mkdirSync(migrationDir, { recursive: true });
  fs.mkdirSync(dockerDir, { recursive: true });

  fs.writeFileSync(
    path.join(apiServerDir, '.env.example'),
    [
      'API_ENV=development',
      'API_DATABASE_URL=postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase',
      'BOOTSTRAP_WORKSPACE_NAME=1flowbase',
      'BOOTSTRAP_ROOT_ACCOUNT=root',
      'BOOTSTRAP_ROOT_EMAIL=root@example.com',
      'BOOTSTRAP_ROOT_PASSWORD=change-me',
    ].join('\n')
  );
  fs.copyFileSync(
    path.resolve(
      __dirname,
      '../../../../api/crates/storage-durable/postgres/migrations/20260808230000_add_user_attribution_to_provider_request_logs.sql'
    ),
    path.join(
      migrationDir,
      '20260808230000_add_user_attribution_to_provider_request_logs.sql'
    )
  );
  fs.writeFileSync(path.join(dockerDir, 'middleware.env'), 'POSTGRES_PORT=35432\n');

  const services = getServiceDefinitions(tempRepoRoot);
  const apiService = services['api-server'];
  ensureServiceEnvFile(apiService);

  const commandCalls = [];
  const composeCalls = [];
  let attempt = 0;

  runServicePrestartCommands(apiService, {
    runCommandImpl(command, args, options) {
      commandCalls.push({ command, args, options });
      attempt += 1;
      if (attempt === 1) {
        return {
          status: 1,
          stdout: '',
          stderr:
            'Error: migration 20260808230000 was previously applied but has been modified\n',
        };
      }

      return {
        status: 0,
        stdout: '',
        stderr: '',
      };
    },
    runMiddlewareComposeImpl(repoRoot, args) {
      composeCalls.push({ repoRoot, args });
      return {
        status: 0,
        stdout: '1\n',
        stderr: '',
      };
    },
  });

  assert.equal(commandCalls.length, 3);
  assert.equal(composeCalls.length, 1);
  assert.equal(composeCalls[0].repoRoot, tempRepoRoot);
  assert.deepEqual(composeCalls[0].args.slice(0, 8), [
    'exec',
    '-T',
    'db',
    'psql',
    '-U',
    'postgres',
    '-d',
    '1flowbase',
  ]);
  assert.ok(composeCalls[0].args.includes('-X'));
  assert.ok(composeCalls[0].args.includes('-A'));
  assert.ok(composeCalls[0].args.includes('-t'));
  assert.ok(composeCalls[0].args.includes('-v'));
  assert.ok(composeCalls[0].args.includes('ON_ERROR_STOP=1'));
  const sql = composeCalls[0].args.at(-1);
  assert.match(sql, /update _sqlx_migrations/iu);
  assert.match(sql, /version = 20260808230000/iu);
  assert.match(
    sql,
    /65656b2b49acc6f0c034d3df7440f5113f08d14613279d55aacc7463948c783c1a1aeb3f667500cd18245bf2a493db66/iu
  );
  assert.match(
    sql,
    /5f8137b467d8d6d16aa5416407b64249ba43e17ea452e37e4811e7b4d7cb5502cd53ad975f2df41615f57dc56e5a9811/iu
  );
  assert.doesNotMatch(sql, /drop database/iu);
});

test('AC-002 refuses the known repair when the database checksum does not match', () => {
  const tempRepoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-refuse-'));
  const apiServerDir = path.join(tempRepoRoot, 'api', 'apps', 'api-server');
  const migrationDir = path.join(
    tempRepoRoot,
    'api',
    'crates',
    'storage-durable',
    'postgres',
    'migrations'
  );
  const dockerDir = path.join(tempRepoRoot, 'docker');

  fs.mkdirSync(apiServerDir, { recursive: true });
  fs.mkdirSync(migrationDir, { recursive: true });
  fs.mkdirSync(dockerDir, { recursive: true });
  fs.writeFileSync(
    path.join(apiServerDir, '.env.example'),
    [
      'API_ENV=development',
      'API_DATABASE_URL=postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase',
    ].join('\n')
  );
  fs.copyFileSync(
    path.resolve(
      __dirname,
      '../../../../api/crates/storage-durable/postgres/migrations/20260808230000_add_user_attribution_to_provider_request_logs.sql'
    ),
    path.join(
      migrationDir,
      '20260808230000_add_user_attribution_to_provider_request_logs.sql'
    )
  );
  fs.writeFileSync(path.join(dockerDir, 'middleware.env'), 'POSTGRES_PORT=35432\n');

  const services = getServiceDefinitions(tempRepoRoot);
  const apiService = services['api-server'];
  ensureServiceEnvFile(apiService);
  const composeCalls = [];

  assert.throws(
    () =>
      runServicePrestartCommands(apiService, {
        logImpl() {},
        runCommandImpl() {
          return {
            status: 1,
            stdout: '',
            stderr:
              'Error: migration 20260808230000 was previously applied but has been modified\n',
          };
        },
        runMiddlewareComposeImpl(repoRoot, args) {
          composeCalls.push({ repoRoot, args });
          return {
            status: 0,
            stdout: '0\n',
            stderr: '',
          };
        },
      }),
    /api-server development frontstage executable upgrade failed with exit code 1/u
  );

  assert.equal(composeCalls.length, 1);
  assert.doesNotMatch(composeCalls[0].args.at(-1), /drop database/iu);
});

test('runServicePrestartCommands rebuilds local postgres db only with explicit reset opt-in', () => {
  const tempRepoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-recover-'));
  const apiServerDir = path.join(tempRepoRoot, 'api', 'apps', 'api-server');
  const dockerDir = path.join(tempRepoRoot, 'docker');

  fs.mkdirSync(apiServerDir, { recursive: true });
  fs.mkdirSync(dockerDir, { recursive: true });

  fs.writeFileSync(
    path.join(apiServerDir, '.env.example'),
    [
      'API_ENV=development',
      'API_DATABASE_URL=postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase',
      'BOOTSTRAP_WORKSPACE_NAME=1flowbase',
      'BOOTSTRAP_ROOT_ACCOUNT=root',
      'BOOTSTRAP_ROOT_EMAIL=root@example.com',
      'BOOTSTRAP_ROOT_PASSWORD=change-me',
    ].join('\n')
  );
  fs.writeFileSync(path.join(dockerDir, 'middleware.env'), 'POSTGRES_PORT=35432\n');

  const services = getServiceDefinitions(tempRepoRoot);
  const apiService = services['api-server'];
  ensureServiceEnvFile(apiService);

  const commandCalls = [];
  const composeCalls = [];
  let attempt = 0;

  runServicePrestartCommands(apiService, {
    sourceEnv: { ONEFLOWBASE_DEV_UP_ALLOW_DB_RESET: '1' },
    runCommandImpl(command, args, options) {
      commandCalls.push({ command, args, options });
      attempt += 1;
      if (attempt === 1) {
        return {
          status: 1,
          stdout: '',
          stderr: 'Error: migration 20260412183000 was previously applied but has been modified\n',
        };
      }

      return {
        status: 0,
        stdout: '',
        stderr: '',
      };
    },
    runMiddlewareComposeImpl(repoRoot, args) {
      composeCalls.push({ repoRoot, args });
      return {
        status: 0,
        stdout: '',
        stderr: '',
      };
    },
  });

  assert.equal(commandCalls.length, 3);
  assert.ok(commandCalls.every((entry) => entry.options.captureOutput === true));
  assert.deepEqual(
    composeCalls.map((entry) => entry.args),
    [
      [
        'exec',
        '-T',
        'db',
        'psql',
        '-U',
        'postgres',
        '-d',
        'postgres',
        '-c',
        'DROP DATABASE IF EXISTS "1flowbase" WITH (FORCE);',
      ],
      [
        'exec',
        '-T',
        'db',
        'psql',
        '-U',
        'postgres',
        '-d',
        'postgres',
        '-c',
        'CREATE DATABASE "1flowbase";',
      ],
    ]
  );
});

test('runServicePrestartCommands lets frontend pnpm prompts write to the terminal', () => {
  const repoRoot = path.resolve(__dirname, '..', '..', '..', '..');
  const services = getServiceDefinitions(repoRoot);
  const commandCalls = [];

  runServicePrestartCommands(services.web, {
    sourceEnv: { CI: 'false' },
    runCommandImpl(command, args, options) {
      commandCalls.push({ command, args, options });
      return {
        status: 0,
        stdout: '',
        stderr: '',
      };
    },
  });

  assert.deepEqual(
    commandCalls.map((entry) => ({
      command: entry.command,
      args: entry.args,
      cwd: entry.options.cwd,
      captureOutput: entry.options.captureOutput,
      ci: entry.options.env.CI,
    })),
    [
      {
        command: 'pnpm',
        args: ['install'],
        cwd: path.join(repoRoot, 'web'),
        captureOutput: false,
        ci: 'false',
      },
    ]
  );
});

test('runServicePrestartCommands rebuilds local postgres db after missing resolved migration drift', () => {
  const tempRepoRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'oneflowbase-dev-up-missing-migration-'));
  const apiServerDir = path.join(tempRepoRoot, 'api', 'apps', 'api-server');
  const dockerDir = path.join(tempRepoRoot, 'docker');

  fs.mkdirSync(apiServerDir, { recursive: true });
  fs.mkdirSync(dockerDir, { recursive: true });

  fs.writeFileSync(
    path.join(apiServerDir, '.env.example'),
    [
      'API_ENV=development',
      'API_DATABASE_URL=postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase',
      'BOOTSTRAP_WORKSPACE_NAME=1flowbase',
      'BOOTSTRAP_ROOT_ACCOUNT=root',
      'BOOTSTRAP_ROOT_EMAIL=root@example.com',
      'BOOTSTRAP_ROOT_PASSWORD=change-me',
    ].join('\n')
  );
  fs.writeFileSync(path.join(dockerDir, 'middleware.env'), 'POSTGRES_PORT=35432\n');

  const services = getServiceDefinitions(tempRepoRoot);
  const apiService = services['api-server'];
  ensureServiceEnvFile(apiService);

  const commandCalls = [];
  const composeCalls = [];
  let attempt = 0;

  runServicePrestartCommands(apiService, {
    sourceEnv: { ONEFLOWBASE_DEV_UP_ALLOW_DB_RESET: '1' },
    runCommandImpl(command, args, options) {
      commandCalls.push({ command, args, options });
      attempt += 1;
      if (attempt === 1) {
        return {
          status: 1,
          stdout: '',
          stderr: 'Error: migration 20260422121000 was previously applied but is missing in the resolved migrations\n',
        };
      }

      return {
        status: 0,
        stdout: '',
        stderr: '',
      };
    },
    runMiddlewareComposeImpl(repoRoot, args) {
      composeCalls.push({ repoRoot, args });
      return {
        status: 0,
        stdout: '',
        stderr: '',
      };
    },
  });

  assert.equal(commandCalls.length, 3);
  assert.ok(commandCalls.every((entry) => entry.options.captureOutput === true));
  assert.deepEqual(
    composeCalls.map((entry) => entry.args),
    [
      [
        'exec',
        '-T',
        'db',
        'psql',
        '-U',
        'postgres',
        '-d',
        'postgres',
        '-c',
        'DROP DATABASE IF EXISTS "1flowbase" WITH (FORCE);',
      ],
      [
        'exec',
        '-T',
        'db',
        'psql',
        '-U',
        'postgres',
        '-d',
        'postgres',
        '-c',
        'CREATE DATABASE "1flowbase";',
      ],
    ]
  );
});
