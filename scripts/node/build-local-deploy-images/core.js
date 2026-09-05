const fs = require('node:fs');
const os = require('node:os');
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
  targetArch,
  targetOs,
  useBuildx,
}) {
  const args = useBuildx
    ? ['buildx', 'build', '--load', '--target', 'runtime']
    : [
      'build',
      '--target',
      'runtime',
      '--build-arg',
      `TARGETOS=${targetOs}`,
      '--build-arg',
      `TARGETARCH=${targetArch}`,
    ];
  args.push('-f', dockerfile, '-t', image, '.');

  const result = runCommand('docker', args, {
    cwd: repoRoot,
    env: { ...process.env, DOCKER_BUILDKIT: useBuildx ? '1' : '0' },
  });
  ensureCommandSuccess(`build ${image}`, result);
}

function localDockerTarget() {
  const architectures = {
    arm64: 'arm64',
    x64: 'amd64',
  };
  const operatingSystems = {
    darwin: 'linux',
    linux: 'linux',
    win32: 'linux',
  };
  const targetArch = architectures[process.arch];
  const targetOs = operatingSystems[process.platform];

  if (!targetArch || !targetOs) {
    throw new Error(`Unsupported local Docker platform: ${process.platform}/${process.arch}`);
  }
  return { targetArch, targetOs };
}

function removeBuildKitRunMounts(source, sourcePath, { cargoJobs } = {}) {
  const lines = source.split('\n');
  const output = [];
  let removingMountPrefix = false;

  for (const line of lines) {
    const firstMount = line.match(/^(\s*)RUN\s+--mount=\S+\s+\\\s*$/u);
    if (firstMount) {
      output.push(`${firstMount[1]}RUN \\`);
      removingMountPrefix = true;
      continue;
    }
    if (removingMountPrefix && /^\s+--mount=\S+\s+\\\s*$/u.test(line)) {
      continue;
    }
    removingMountPrefix = false;
    output.push(line);
  }

  let transformed = output.join('\n');
  if (cargoJobs) {
    transformed = transformed.replace(
      /RUN \\\n(\s*)CARGO_TARGET_DIR=/u,
      `RUN cargo fetch --locked\n\nRUN \\\n$1CARGO_BUILD_JOBS=${cargoJobs} CARGO_TARGET_DIR=`,
    );
  }
  if (/(?:^|\s)--mount=/mu.test(transformed)) {
    throw new Error(`Cannot create legacy-compatible Dockerfile from ${sourcePath}`);
  }
  return transformed;
}

function recommendedCargoJobs({ cpuCount = os.cpus().length, totalMemory = os.totalmem() } = {}) {
  const memoryBoundJobs = Math.max(1, Math.floor(totalMemory / (6 * 1024 ** 3)));
  return Math.max(1, Math.min(cpuCount, memoryBoundJobs));
}

function createLegacyDockerfiles(repoRoot) {
  const tempRoot = path.join(repoRoot, 'tmp');
  fs.mkdirSync(tempRoot, { recursive: true });
  const tempDir = fs.mkdtempSync(path.join(tempRoot, 'local-deploy-images-'));
  const dockerfiles = {};

  for (const [component, filename] of Object.entries({
    apiServer: 'api-server.Dockerfile',
    web: 'web.Dockerfile',
  })) {
    const sourcePath = path.join(repoRoot, 'docker', filename);
    const targetPath = path.join(tempDir, filename);
    const source = fs.readFileSync(sourcePath, 'utf8');
    const cargoJobs = component === 'apiServer' ? recommendedCargoJobs() : undefined;
    fs.writeFileSync(targetPath, removeBuildKitRunMounts(source, sourcePath, { cargoJobs }));
    dockerfiles[component] = targetPath;
  }

  return { dockerfiles, tempDir };
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
  log(
    `builder: ${useBuildx ? 'docker buildx build --load' : 'local Docker legacy builder without cache mounts'}`,
    writeStdout,
  );

  const legacyFiles = useBuildx ? null : createLegacyDockerfiles(repoRoot);
  const dockerfiles = legacyFiles?.dockerfiles || {
    apiServer: 'docker/api-server.Dockerfile',
    web: 'docker/web.Dockerfile',
  };
  const target = localDockerTarget();
  if (!useBuildx) {
    log(`legacy Cargo compile jobs: ${recommendedCargoJobs()}`, writeStdout);
  }

  try {
    log(`building ${images.apiServer}`, writeStdout);
    buildImage({
      dockerfile: dockerfiles.apiServer,
      image: images.apiServer,
      repoRoot,
      runCommand,
      ...target,
      useBuildx,
    });

    log(`building ${images.web}`, writeStdout);
    buildImage({
      dockerfile: dockerfiles.web,
      image: images.web,
      repoRoot,
      runCommand,
      ...target,
      useBuildx,
    });
  } finally {
    if (legacyFiles) {
      fs.rmSync(legacyFiles.tempDir, { force: true, recursive: true });
    }
  }

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
  createLegacyDockerfiles,
  ensureDeployEnv,
  main,
  removeBuildKitRunMounts,
  readImageVersions,
  recommendedCargoJobs,
  runLocalDeployImageBuild,
  usage,
};
