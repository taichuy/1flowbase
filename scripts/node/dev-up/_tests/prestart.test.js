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

test('getServicePrestartCommands resets api root password in development mode', () => {
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
      command: command.command,
      args: command.args,
      cwd: command.cwd,
    })),
    [
      {
        command: 'cargo',
        args: ['run', '-p', 'api-server', '--bin', 'reset_root_password'],
        cwd: path.join(tempRepoRoot, 'api'),
      },
    ]
  );
  assert.equal(commands[0].env.API_ENV, 'development');
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

test('runServicePrestartCommands blocks local postgres rebuild by default after migration checksum mismatch', () => {
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
      /api-server development root password reset failed with exit code 1/u
    );
  } finally {
    process.stderr.write = originalStderrWrite;
  }

  assert.equal(commandCalls.length, 1);
  assert.equal(commandCalls[0].options.captureOutput, true);
  assert.deepEqual(composeCalls, []);
  assert.equal(
    stderrOutput.match(/previously applied but has been modified/gu)?.length,
    1
  );
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

  assert.equal(commandCalls.length, 2);
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

  assert.equal(commandCalls.length, 2);
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
