const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const { log } = require('./cli.js');
const {
  buildLocalLoopbackEnv,
  commandExists,
  parseEnvFile,
  resolveCommandPath,
} = require('./env.js');
const { getRepoRoot } = require('./services.js');

const DEFAULT_MIDDLEWARE_HOST_PORTS = {
  POSTGRES_PORT: 35432,
};

function runCommand(command, args, options = {}) {
  const resolvedCommand = resolveCommandPath(command) || command;
  const extension = path.extname(resolvedCommand).toLowerCase();
  return spawnSync(resolvedCommand, args, {
    cwd: options.cwd || getRepoRoot(),
    env: { ...buildLocalLoopbackEnv(process.env), ...(options.env || {}) },
    encoding: 'utf8',
    shell: process.platform === 'win32' && (extension === '.cmd' || extension === '.bat'),
    stdio: options.captureOutput ? ['ignore', 'pipe', 'pipe'] : 'inherit',
  });
}

function ensureCommandSuccess(description, result) {
  if (!result.error && result.status === 0) {
    return;
  }

  if (result.stdout) {
    process.stdout.write(result.stdout);
  }

  if (result.stderr) {
    process.stderr.write(result.stderr);
  }

  if (result.error) {
    throw result.error;
  }

  throw new Error(`${description} 失败，退出码 ${result.status}`);
}

function writeCommandOutput(result) {
  if (result?.stdout) {
    process.stdout.write(result.stdout);
  }

  if (result?.stderr) {
    process.stderr.write(result.stderr);
  }
}

let cachedComposeCommand = null;

function resolveComposeCommand({ resetCache = false, runCommandImpl = runCommand } = {}) {
  if (resetCache) {
    cachedComposeCommand = null;
  }

  if (cachedComposeCommand) {
    return cachedComposeCommand;
  }

  const dockerComposeResult = runCommandImpl('docker', ['compose', 'version'], {
    captureOutput: true,
  });
  if (!dockerComposeResult.error && dockerComposeResult.status === 0) {
    cachedComposeCommand = { command: 'docker', baseArgs: ['compose'] };
    return cachedComposeCommand;
  }

  const standaloneComposeResult = runCommandImpl('docker-compose', ['version'], {
    captureOutput: true,
  });
  const standaloneComposeOutput = `${standaloneComposeResult.stdout || ''}\n${
    standaloneComposeResult.stderr || ''
  }`;
  if (
    !standaloneComposeResult.error &&
    standaloneComposeResult.status === 0 &&
    /\bCompose version v?2\./iu.test(standaloneComposeOutput)
  ) {
    cachedComposeCommand = { command: 'docker-compose', baseArgs: [] };
    return cachedComposeCommand;
  }

  throw new Error('缺少 `docker compose` 命令或 Docker Compose v2 版 `docker-compose` 命令');
}

function ensureMiddlewareEnv(repoRoot, { logImpl = log } = {}) {
  const dockerDir = path.join(repoRoot, 'docker');
  const examplePath = path.join(dockerDir, 'middleware.env.example');
  const targetPath = path.join(dockerDir, 'middleware.env');

  if (!fs.existsSync(targetPath) && fs.existsSync(examplePath)) {
    fs.copyFileSync(examplePath, targetPath);
    logImpl('已创建 docker/middleware.env');
  }
}

function runMiddlewareCompose(repoRoot, args, options = {}) {
  const composeCommand = resolveComposeCommand();
  const result = runCommand(
    composeCommand.command,
    [...composeCommand.baseArgs, '-f', 'docker-compose.middleware.yaml', ...args],
    {
      cwd: path.join(repoRoot, 'docker'),
      captureOutput: options.captureOutput === true,
    }
  );

  if (options.allowFailure === true) {
    return result;
  }

  ensureCommandSuccess(`docker 中间件命令 ${args.join(' ')}`, result);
  return result;
}

function listPortOccupantPids(port, { runCommandImpl = runCommand } = {}) {
  if (!Number.isInteger(port) || port <= 0 || !commandExists('lsof')) {
    return [];
  }

  const result = runCommandImpl('lsof', ['-t', `-iTCP:${port}`, '-sTCP:LISTEN', '-P', '-n'], {
    captureOutput: true,
  });
  if (result.error || result.status !== 0) {
    return [];
  }

  return String(result.stdout || '')
    .split(/\r?\n/)
    .map((value) => Number.parseInt(value.trim(), 10))
    .filter((value) => Number.isInteger(value) && value > 0);
}

async function waitForProcessExit(pid, timeoutMs = 5000) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    try {
      process.kill(pid, 0);
      await new Promise((resolve) => setTimeout(resolve, 200));
    } catch (error) {
      if (error.code === 'ESRCH') {
        return true;
      }

      throw error;
    }
  }

  try {
    process.kill(pid, 0);
    return false;
  } catch (error) {
    if (error.code === 'ESRCH') {
      return true;
    }

    throw error;
  }
}

async function clearPortConflicts(
  label,
  ports,
  {
    listPortOccupantPidsImpl = listPortOccupantPids,
    waitForProcessExitImpl = waitForProcessExit,
    logImpl = log,
  } = {}
) {
  const normalizedPorts = [...new Set(ports.filter((port) => Number.isInteger(port) && port > 0))];

  for (const port of normalizedPorts) {
    const occupants = listPortOccupantPidsImpl(port);
    if (occupants.length === 0) {
      continue;
    }

    logImpl(`${label} 检测到端口 ${port} 被其他进程占用，正在清理 pid=${occupants.join(',')}`);

    for (const pid of occupants) {
      try {
        process.kill(pid, 'SIGTERM');
      } catch (error) {
        if (error.code !== 'ESRCH') {
          throw error;
        }
      }
    }

    for (const pid of occupants) {
      const exited = await waitForProcessExitImpl(pid);
      if (exited) {
        continue;
      }

      try {
        process.kill(pid, 'SIGKILL');
      } catch (error) {
        if (error.code !== 'ESRCH') {
          throw error;
        }
      }
      await waitForProcessExitImpl(pid, 2000);
    }
  }
}

function getMiddlewareHostPorts(repoRoot) {
  const envPath = path.join(repoRoot, 'docker', 'middleware.env');
  const env = parseEnvFile(envPath);

  return Object.entries(DEFAULT_MIDDLEWARE_HOST_PORTS).map(([key, defaultPort]) => {
    const configured = Number.parseInt(env[key] ?? '', 10);
    return Number.isInteger(configured) && configured > 0 ? configured : defaultPort;
  });
}

async function manageDocker(
  repoRoot,
  action,
  {
    ensureMiddlewareEnvImpl = ensureMiddlewareEnv,
    runMiddlewareComposeImpl = runMiddlewareCompose,
    getMiddlewareHostPortsImpl = getMiddlewareHostPorts,
    clearPortConflictsImpl = clearPortConflicts,
  } = {}
) {
  ensureMiddlewareEnvImpl(repoRoot);

  if (action === 'status') {
    const result = runMiddlewareComposeImpl(repoRoot, ['ps'], {
      captureOutput: true,
      allowFailure: true,
    });

    writeCommandOutput(result);

    if (result.error || result.status !== 0) {
      throw new Error('docker 中间件状态检查失败');
    }
    return;
  }

  if (action === 'stop') {
    runMiddlewareComposeImpl(repoRoot, ['down']);
    return;
  }

  if (action === 'restart') {
    runMiddlewareComposeImpl(repoRoot, ['down']);
    await clearPortConflictsImpl('docker 中间件', getMiddlewareHostPortsImpl(repoRoot));
  }

  runMiddlewareComposeImpl(repoRoot, ['up', '-d']);
}

function getMiddlewarePostgresPort(repoRoot) {
  const dockerDir = path.join(repoRoot, 'docker');
  for (const fileName of ['middleware.env', 'middleware.env.example']) {
    const fileEnv = parseEnvFile(path.join(dockerDir, fileName));
    if (fileEnv.POSTGRES_PORT) {
      return String(fileEnv.POSTGRES_PORT);
    }
  }

  return '35432';
}

module.exports = {
  clearPortConflicts,
  ensureCommandSuccess,
  ensureMiddlewareEnv,
  getMiddlewareHostPorts,
  getMiddlewarePostgresPort,
  manageDocker,
  resolveComposeCommand,
  runCommand,
  runMiddlewareCompose,
  writeCommandOutput,
};
