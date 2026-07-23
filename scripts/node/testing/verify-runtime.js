const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const LOCAL_VERIFY_CONFIG_FILE = ".1flowbase.verify.local.json";
const VERIFY_LOCK_TOKEN_ENV = "ONEFLOWBASE_VERIFY_LOCK_TOKEN";
const HEAVY_VERIFY_LOCK_DIR = path.join(
  "tmp",
  "test-governance",
  "locks",
  "heavy-verify",
);
const DEFAULT_WAIT_TIMEOUT_MINUTES = 30;
const DEFAULT_POLL_INTERVAL_MS = 5000;

function getAvailableParallelism() {
  if (typeof os.availableParallelism === "function") {
    return os.availableParallelism();
  }

  return os.cpus().length;
}

function isCiEnvironment(env = process.env) {
  return (
    isTruthyEnvironmentValue(env.CI) ||
    isTruthyEnvironmentValue(env.GITHUB_ACTIONS)
  );
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function isTruthyEnvironmentValue(value) {
  if (value === true || value === 1) {
    return true;
  }

  if (typeof value !== "string") {
    return false;
  }

  const normalized = value.trim().toLowerCase();
  return (
    normalized === "true" ||
    normalized === "1" ||
    normalized === "yes" ||
    normalized === "on"
  );
}

function assertPlainObject(name, value) {
  if (!isPlainObject(value)) {
    throw new Error(`${name} must be a plain object`);
  }

  return value;
}

function assertPositiveInteger(name, value) {
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${name} must be a positive integer`);
  }

  return value;
}

function assertKnownKeys(name, value, allowedKeys) {
  const unknownKeys = Object.keys(value)
    .filter((key) => !allowedKeys.has(key))
    .sort();

  if (unknownKeys.length > 0) {
    throw new Error(
      `Unknown ${name} key${unknownKeys.length === 1 ? "" : "s"}: ${unknownKeys.join(", ")}`,
    );
  }
}

function resolveCargoDefaults(availableParallelism) {
  const parallelism = assertPositiveInteger(
    "availableParallelism",
    availableParallelism,
  );

  return {
    cargoJobs: parallelism,
    cargoTestThreads: parallelism,
  };
}

function resolveFrontendDefaults(availableParallelism) {
  const parallelism = assertPositiveInteger(
    "availableParallelism",
    availableParallelism,
  );

  return {
    turboConcurrency: parallelism,
    vitestMaxWorkers: parallelism,
  };
}

function readLocalVerifyConfig(repoRoot) {
  const configPath = path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE);

  if (!fs.existsSync(configPath)) {
    return undefined;
  }

  const raw = fs.readFileSync(configPath, "utf8");

  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(
      `Failed to parse ${LOCAL_VERIFY_CONFIG_FILE}: ${error.message}`,
    );
  }
}

function getHeavyVerifyLockDir(repoRoot) {
  return path.join(repoRoot, HEAVY_VERIFY_LOCK_DIR);
}

function getHeavyVerifyLockOwnerPath(repoRoot) {
  return path.join(getHeavyVerifyLockDir(repoRoot), "owner.json");
}

function getHeavyVerifyLockStagingDir(repoRoot, processId) {
  const parentDir = path.dirname(getHeavyVerifyLockDir(repoRoot));
  return path.join(
    parentDir,
    `heavy-verify-${processId}-${crypto.randomUUID()}.tmp`,
  );
}

function isValidHeavyVerifyLockOwner(owner) {
  return (
    isPlainObject(owner) &&
    typeof owner.token === "string" &&
    owner.token.length > 0 &&
    Number.isInteger(owner.pid) &&
    owner.pid > 0 &&
    typeof owner.scope === "string" &&
    owner.scope.length > 0 &&
    typeof owner.command === "string" &&
    owner.command.length > 0 &&
    typeof owner.cwd === "string" &&
    owner.cwd.length > 0 &&
    typeof owner.startedAt === "string" &&
    owner.startedAt.length > 0 &&
    typeof owner.hostname === "string" &&
    owner.hostname.length > 0
  );
}

function readHeavyVerifyLockOwner({ repoRoot } = {}) {
  if (typeof repoRoot !== "string" || repoRoot.trim() === "") {
    throw new Error("repoRoot must be a non-empty string");
  }

  const ownerPath = getHeavyVerifyLockOwnerPath(repoRoot);

  if (!fs.existsSync(ownerPath)) {
    return null;
  }

  try {
    const owner = JSON.parse(fs.readFileSync(ownerPath, "utf8"));
    return isValidHeavyVerifyLockOwner(owner) ? owner : null;
  } catch (_error) {
    return null;
  }
}

function getVerifyLockToken(env = process.env) {
  const token = env?.[VERIFY_LOCK_TOKEN_ENV];

  if (typeof token === "string" && token.trim() !== "") {
    return token;
  }

  return typeof crypto.randomUUID === "function"
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function writeHeavyVerifyLockOwner({
  lockDir,
  token,
  scope,
  command,
  cwd,
  startedAt,
  pid,
  hostname,
}) {
  const ownerPath = path.join(lockDir, "owner.json");

  fs.writeFileSync(
    ownerPath,
    `${JSON.stringify(
      {
        token,
        pid,
        scope,
        command,
        cwd,
        startedAt,
        hostname,
      },
      null,
      2,
    )}\n`,
  );

  return {
    token,
    pid,
    scope,
    command,
    cwd,
    startedAt,
    hostname,
    ownerPath,
  };
}

function removeHeavyVerifyLock({ repoRoot, token }) {
  const owner = readHeavyVerifyLockOwner({ repoRoot });

  if (owner !== null && owner.token !== token) {
    return false;
  }

  fs.rmSync(getHeavyVerifyLockDir(repoRoot), { recursive: true, force: true });
  return true;
}

function isHeavyVerifyLockPublishCollision(error) {
  return (
    error &&
    (error.code === "EEXIST" ||
      error.code === "ENOTEMPTY" ||
      error.code === "EPERM")
  );
}

function defaultForwardFatalCleanupEvent(eventName, payload) {
  if (eventName === "SIGINT" || eventName === "SIGTERM") {
    setImmediate(() => {
      process.kill(process.pid, eventName);
    });
    return;
  }

  setImmediate(() => {
    if (payload instanceof Error) {
      throw payload;
    }

    throw new Error(
      eventName === "unhandledRejection"
        ? `unhandledRejection: ${String(payload)}`
        : String(payload),
    );
  });
}

async function acquireHeavyVerifyLock({
  repoRoot,
  env = process.env,
  scope,
  command,
  runtimeConfig = loadVerifyRuntimeConfig({ repoRoot, env }),
  writeStdout = (text) => process.stdout.write(text),
  sleepImpl = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  isProcessAliveImpl = (pid) => {
    try {
      process.kill(pid, 0);
      return true;
    } catch (error) {
      if (error.code === "ESRCH") {
        return false;
      }

      throw error;
    }
  },
  now = () => new Date(),
  hostname = os.hostname(),
  processId = process.pid,
  beforeOwnerPublishImpl = async () => {},
} = {}) {
  if (typeof repoRoot !== "string" || repoRoot.trim() === "") {
    throw new Error("repoRoot must be a non-empty string");
  }

  if (typeof scope !== "string" || scope.trim() === "") {
    throw new Error("scope must be a non-empty string");
  }

  if (typeof command !== "string" || command.trim() === "") {
    throw new Error("command must be a non-empty string");
  }

  assertPlainObject("runtimeConfig", runtimeConfig);
  assertPlainObject("runtimeConfig.backend", runtimeConfig.backend);
  assertPlainObject("runtimeConfig.locks", runtimeConfig.locks);
  assertPositiveInteger(
    "runtimeConfig.locks.waitTimeoutMs",
    runtimeConfig.locks.waitTimeoutMs,
  );
  assertPositiveInteger(
    "runtimeConfig.locks.pollIntervalMs",
    runtimeConfig.locks.pollIntervalMs,
  );

  const lockDir = getHeavyVerifyLockDir(repoRoot);
  const token = getVerifyLockToken(env);
  const startedAtDate = now();
  const startedAt = startedAtDate.toISOString();
  const timeoutAt = startedAtDate.getTime() + runtimeConfig.locks.waitTimeoutMs;
  const stagingDir = getHeavyVerifyLockStagingDir(repoRoot, processId);
  const ownerRecord = {
    token,
    pid: processId,
    scope,
    command,
    cwd: repoRoot,
    startedAt,
    hostname,
  };

  fs.mkdirSync(path.dirname(lockDir), { recursive: true });

  let released = false;

  const lock = {
    token,
    reentrant: false,
    release() {
      if (released) {
        return false;
      }

      released = true;
      const removed = removeHeavyVerifyLock({ repoRoot, token });

      if (removed) {
        writeStdout(
          `[1flowbase-verify-lock] released: scope=${scope} pid=${processId} token=${token}\n`,
        );
      }

      return removed;
    },
  };

  while (true) {
    let published = false;
    fs.mkdirSync(stagingDir);

    try {
      writeHeavyVerifyLockOwner({
        lockDir: stagingDir,
        ...ownerRecord,
      });

      await beforeOwnerPublishImpl({
        repoRoot,
        scope,
        command,
        token,
        pid: processId,
        stagingDir,
      });

      const lockDirExists = fs.existsSync(lockDir);
      const visibleOwner = lockDirExists
        ? readHeavyVerifyLockOwner({ repoRoot })
        : null;

      if (lockDirExists) {
        if (visibleOwner === null) {
          writeStdout(
            "[1flowbase-verify-lock] stale lock detected, cleaning...\n",
          );
          fs.rmSync(lockDir, { recursive: true, force: true });
          continue;
        }

        if (visibleOwner.token === token) {
          writeStdout(
            `[1flowbase-verify-lock] reentrant: scope=${scope} pid=${processId} token=${token}\n`,
          );
          return {
            token,
            reentrant: true,
            release() {
              return false;
            },
          };
        }

        if (!isProcessAliveImpl(visibleOwner.pid)) {
          writeStdout(
            "[1flowbase-verify-lock] stale lock detected, cleaning...\n",
          );
          fs.rmSync(lockDir, { recursive: true, force: true });
          continue;
        }

        writeStdout(
          `[1flowbase-verify-lock] busy: scope=${visibleOwner.scope} pid=${visibleOwner.pid} startedAt=${visibleOwner.startedAt}\n`,
        );

        if (now().getTime() >= timeoutAt) {
          writeStdout(
            `[1flowbase-verify-lock] timeout waiting for heavy-verify lock: scope=${visibleOwner.scope} pid=${visibleOwner.pid} token=${visibleOwner.token}\n`,
          );
          throw new Error("timeout waiting for heavy-verify lock");
        }

        writeStdout("[1flowbase-verify-lock] waiting...\n");
        await sleepImpl(runtimeConfig.locks.pollIntervalMs);
        continue;
      }

      fs.renameSync(stagingDir, lockDir);
      published = true;
      writeStdout(
        `[1flowbase-verify-lock] acquired: scope=${scope} pid=${processId} token=${token}\n`,
      );
      return lock;
    } catch (error) {
      if (!isHeavyVerifyLockPublishCollision(error)) {
        fs.rmSync(stagingDir, { recursive: true, force: true });
        throw error;
      }
    } finally {
      if (!published) {
        fs.rmSync(stagingDir, { recursive: true, force: true });
      }
    }

    const owner = readHeavyVerifyLockOwner({ repoRoot });

    if (owner?.token === token) {
      writeStdout(
        `[1flowbase-verify-lock] reentrant: scope=${scope} pid=${processId} token=${token}\n`,
      );
      return {
        token,
        reentrant: true,
        release() {
          return false;
        },
      };
    }

    if (owner === null) {
      writeStdout("[1flowbase-verify-lock] stale lock detected, cleaning...\n");
      fs.rmSync(lockDir, { recursive: true, force: true });
      continue;
    }

    if (!isProcessAliveImpl(owner.pid)) {
      writeStdout("[1flowbase-verify-lock] stale lock detected, cleaning...\n");
      fs.rmSync(lockDir, { recursive: true, force: true });
      continue;
    }

    writeStdout(
      `[1flowbase-verify-lock] busy: scope=${owner.scope} pid=${owner.pid} startedAt=${owner.startedAt}\n`,
    );

    if (now().getTime() >= timeoutAt) {
      writeStdout(
        `[1flowbase-verify-lock] timeout waiting for heavy-verify lock: scope=${owner.scope} pid=${owner.pid} token=${owner.token}\n`,
      );
      throw new Error("timeout waiting for heavy-verify lock");
    }

    writeStdout("[1flowbase-verify-lock] waiting...\n");
    await sleepImpl(runtimeConfig.locks.pollIntervalMs);
  }
}

async function withHeavyVerifyLock(options = {}, run) {
  const lock = await acquireHeavyVerifyLock(options);
  const baseEnv = options.env ?? process.env;
  const callbackEnv = {
    ...baseEnv,
    [VERIFY_LOCK_TOKEN_ENV]: lock.token,
  };
  const processEmitter = options.processEmitter ?? process;
  const forwardFatalCleanupEvent =
    options.forwardFatalCleanupEvent ??
    (processEmitter === process ? defaultForwardFatalCleanupEvent : () => {});
  const cleanupEvents = [
    "SIGINT",
    "SIGTERM",
    "uncaughtException",
    "unhandledRejection",
  ];
  const cleanupHandlers = cleanupEvents.map((eventName) => {
    const handler = (payload) => {
      lock.release();
      forwardFatalCleanupEvent(eventName, payload);
    };

    if (typeof processEmitter.once === "function") {
      processEmitter.once(eventName, handler);
    } else if (typeof processEmitter.on === "function") {
      processEmitter.on(eventName, handler);
    } else {
      throw new Error("processEmitter must support once or on");
    }

    return { eventName, handler };
  });

  try {
    return await run(callbackEnv);
  } finally {
    const remover =
      typeof processEmitter.removeListener === "function"
        ? processEmitter.removeListener.bind(processEmitter)
        : typeof processEmitter.off === "function"
          ? processEmitter.off.bind(processEmitter)
          : null;

    if (remover !== null) {
      for (const { eventName, handler } of cleanupHandlers) {
        remover(eventName, handler);
      }
    }

    lock.release();
  }
}

function resolveRuntimeConfig(config, availableParallelism) {
  assertPlainObject("verify runtime config root", config);
  assertKnownKeys(
    "verify runtime config",
    config,
    new Set(["backend", "frontend", "locks"]),
  );
  const backendDefaults = resolveCargoDefaults(availableParallelism);
  const frontendDefaults = resolveFrontendDefaults(availableParallelism);
  const backendConfig =
    config.backend === undefined
      ? {}
      : assertPlainObject("backend", config.backend);
  const frontendConfig =
    config.frontend === undefined
      ? {}
      : assertPlainObject("frontend", config.frontend);
  const locksConfig =
    config.locks === undefined ? {} : assertPlainObject("locks", config.locks);

  assertKnownKeys(
    "backend",
    backendConfig,
    new Set(["cargoJobs", "cargoTestThreads"]),
  );
  assertKnownKeys(
    "frontend",
    frontendConfig,
    new Set(["turboConcurrency", "vitestMaxWorkers"]),
  );
  assertKnownKeys(
    "locks",
    locksConfig,
    new Set(["waitTimeoutMinutes", "pollIntervalMs"]),
  );

  const cargoJobs = backendConfig.cargoJobs ?? backendDefaults.cargoJobs;
  const cargoTestThreads =
    backendConfig.cargoTestThreads ?? backendDefaults.cargoTestThreads;
  const turboConcurrency =
    frontendConfig.turboConcurrency ?? frontendDefaults.turboConcurrency;
  const vitestMaxWorkers =
    frontendConfig.vitestMaxWorkers ?? frontendDefaults.vitestMaxWorkers;
  const waitTimeoutMinutes =
    locksConfig.waitTimeoutMinutes ?? DEFAULT_WAIT_TIMEOUT_MINUTES;
  const pollIntervalMs = locksConfig.pollIntervalMs ?? DEFAULT_POLL_INTERVAL_MS;

  assertPositiveInteger("backend.cargoJobs", cargoJobs);
  assertPositiveInteger("backend.cargoTestThreads", cargoTestThreads);
  assertPositiveInteger("frontend.turboConcurrency", turboConcurrency);
  assertPositiveInteger("frontend.vitestMaxWorkers", vitestMaxWorkers);
  assertPositiveInteger("locks.waitTimeoutMinutes", waitTimeoutMinutes);
  assertPositiveInteger("locks.pollIntervalMs", pollIntervalMs);

  if (cargoJobs > availableParallelism) {
    throw new Error("backend.cargoJobs must not exceed availableParallelism");
  }

  if (cargoTestThreads > availableParallelism) {
    throw new Error(
      "backend.cargoTestThreads must not exceed availableParallelism",
    );
  }

  if (turboConcurrency > availableParallelism) {
    throw new Error(
      "frontend.turboConcurrency must not exceed availableParallelism",
    );
  }

  if (vitestMaxWorkers > availableParallelism) {
    throw new Error(
      "frontend.vitestMaxWorkers must not exceed availableParallelism",
    );
  }

  return {
    backend: {
      cargoJobs,
      cargoTestThreads,
    },
    frontend: {
      turboConcurrency,
      vitestMaxWorkers,
    },
    locks: {
      waitTimeoutMinutes,
      waitTimeoutMs: waitTimeoutMinutes * 60 * 1000,
      pollIntervalMs,
    },
  };
}

function loadVerifyRuntimeConfig({
  repoRoot,
  env = process.env,
  availableParallelism = getAvailableParallelism(),
} = {}) {
  if (typeof repoRoot !== "string" || repoRoot.trim() === "") {
    throw new Error("repoRoot must be a non-empty string");
  }

  if (isCiEnvironment(env)) {
    return resolveRuntimeConfig(
      {
        backend: {
          cargoJobs: availableParallelism,
          cargoTestThreads: availableParallelism,
        },
        frontend: {
          turboConcurrency: availableParallelism,
          vitestMaxWorkers: availableParallelism,
        },
      },
      availableParallelism,
    );
  }

  const config = readLocalVerifyConfig(repoRoot);
  if (config === undefined) {
    return resolveRuntimeConfig({}, availableParallelism);
  }

  return resolveRuntimeConfig(config, availableParallelism);
}

function main(argv = [], deps = {}) {
  if (argv.length !== 1 || argv[0] !== "cargo-jobs") {
    throw new Error(
      "Usage: node scripts/node/testing/verify-runtime.js cargo-jobs",
    );
  }

  const repoRoot = deps.repoRoot ?? path.resolve(__dirname, "../../..");
  const runtimeConfig = loadVerifyRuntimeConfig({
    repoRoot,
    env: deps.env ?? process.env,
    availableParallelism:
      deps.availableParallelism ?? getAvailableParallelism(),
  });
  const writeStdout = deps.writeStdout ?? ((text) => process.stdout.write(text));

  writeStdout(`${runtimeConfig.backend.cargoJobs}\n`);
  return 0;
}

if (require.main === module) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`[1flowbase-verify-runtime] ${error.message}\n`);
    process.exitCode = 1;
  }
}

module.exports = {
  HEAVY_VERIFY_LOCK_DIR,
  LOCAL_VERIFY_CONFIG_FILE,
  VERIFY_LOCK_TOKEN_ENV,
  acquireHeavyVerifyLock,
  getAvailableParallelism,
  isCiEnvironment,
  loadVerifyRuntimeConfig,
  main,
  readHeavyVerifyLockOwner,
  withHeavyVerifyLock,
};
