const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const SCRIPT_LABEL = '1flowbase-build-local-deploy-images';
const IMAGE_REPOSITORIES = {
  apiServer: 'ghcr.io/taichuy/1flowbase-api-server',
  web: 'ghcr.io/taichuy/1flowbase-web',
};

function getRepoRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function usage(writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(`Usage: node scripts/node/build-local-deploy-images.js

Builds the current API and web source into the image tags configured in deploy/docker/.env.
If deploy/docker/.env is missing, it is copied from deploy/docker/.env.example.
This command builds images only. It never starts or recreates Docker Compose services.
`);
}

function ensureDeployEnv(deployDir) {
  const envPath = path.join(deployDir, '.env');
  if (fs.existsSync(envPath)) {
    return { created: false, envPath };
  }

  const examplePath = path.join(deployDir, '.env.example');
  if (!fs.existsSync(examplePath)) {
    throw new Error(`Missing deployment environment example: ${examplePath}`);
  }

  fs.copyFileSync(examplePath, envPath, fs.constants.COPYFILE_EXCL);
  return { created: true, envPath };
}

function parseEnvValue(source, key) {
  const escapedKey = key.replace(/[.*+?^${}()|[\]\\]/gu, '\\$&');
  const match = source.match(new RegExp(`^\\s*(?:export\\s+)?${escapedKey}\\s*=\\s*(.*?)\\s*$`, 'mu'));
  if (!match) {
    return null;
  }

  const value = match[1];
  if (
    value.length >= 2
    && ((value.startsWith('"') && value.endsWith('"'))
      || (value.startsWith("'") && value.endsWith("'")))
  ) {
    return value.slice(1, -1);
  }
  return value;
}

function assertImageTag(name, value) {
  if (!/^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$/u.test(value)) {
    throw new Error(`${name} is not a valid Docker image tag: ${JSON.stringify(value)}`);
  }
}

function readImageVersions(envPath) {
  const source = fs.readFileSync(envPath, 'utf8');
  const versions = {
    apiServer: parseEnvValue(source, 'FLOWBASE_API_SERVER_VERSION') || 'latest',
    web: parseEnvValue(source, 'FLOWBASE_WEB_VERSION') || 'latest',
  };

  assertImageTag('FLOWBASE_API_SERVER_VERSION', versions.apiServer);
  assertImageTag('FLOWBASE_WEB_VERSION', versions.web);
  return versions;
}

function defaultRunCommand(command, args, options = {}) {
  return spawnSync(command, args, {
    cwd: options.cwd,
    encoding: 'utf8',
    env: options.env || process.env,
    stdio: options.captureOutput ? ['ignore', 'pipe', 'pipe'] : 'inherit',
  });
}

function ensureCommandSuccess(label, result) {
  if (result.error) {
    throw new Error(`${label} failed: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(`${label} failed with exit code ${result.status ?? 1}`);
  }
}

function log(message, writeStdout = (text) => process.stdout.write(text)) {
  writeStdout(`[${SCRIPT_LABEL}] ${message}\n`);
}

function buildImage({
  dockerfile,
  image,
  repoRoot,
  runCommand,
  useBuildx,
}) {
  const args = useBuildx
    ? ['buildx', 'build', '--load', '--target', 'runtime']
    : ['build', '--target', 'runtime'];
  args.push('-f', dockerfile, '-t', image, '.');

  const result = runCommand('docker', args, {
    cwd: repoRoot,
    env: { ...process.env, DOCKER_BUILDKIT: '1' },
  });
  ensureCommandSuccess(`build ${image}`, result);
}

function runLocalDeployImageBuild({
  deployDir,
  repoRoot = getRepoRoot(),
  runCommand = defaultRunCommand,
  writeStdout = (text) => process.stdout.write(text),
} = {}) {
  const selectedDeployDir = deployDir || path.join(repoRoot, 'deploy', 'docker');
  const envResult = ensureDeployEnv(selectedDeployDir);
  const versions = readImageVersions(envResult.envPath);
  const images = {
    apiServer: `${IMAGE_REPOSITORIES.apiServer}:${versions.apiServer}`,
    web: `${IMAGE_REPOSITORIES.web}:${versions.web}`,
  };

  if (envResult.created) {
    log(`created ${path.relative(repoRoot, envResult.envPath)} from .env.example`, writeStdout);
  } else {
    log(`preserving existing ${path.relative(repoRoot, envResult.envPath)}`, writeStdout);
  }

  const buildxResult = runCommand('docker', ['buildx', 'version'], {
    captureOutput: true,
    cwd: repoRoot,
    env: process.env,
  });
  const useBuildx = !buildxResult.error && buildxResult.status === 0;
  log(`builder: ${useBuildx ? 'docker buildx build --load' : 'docker build with BuildKit'}`, writeStdout);

  log(`building ${images.apiServer}`, writeStdout);
  buildImage({
    dockerfile: 'docker/api-server.Dockerfile',
    image: images.apiServer,
    repoRoot,
    runCommand,
    useBuildx,
  });

  log(`building ${images.web}`, writeStdout);
  buildImage({
    dockerfile: 'docker/web.Dockerfile',
    image: images.web,
    repoRoot,
    runCommand,
    useBuildx,
  });

  log('images built; run docker-compose up manually when ready', writeStdout);
  return 0;
}

function main(argv = [], deps = {}) {
  if (argv.includes('-h') || argv.includes('--help')) {
    usage(deps.writeStdout);
    return 0;
  }
  if (argv.length > 0) {
    throw new Error(`Unknown argument: ${argv[0]}`);
  }
  return runLocalDeployImageBuild(deps);
}

module.exports = {
  ensureDeployEnv,
  main,
  readImageVersions,
  runLocalDeployImageBuild,
  usage,
};
