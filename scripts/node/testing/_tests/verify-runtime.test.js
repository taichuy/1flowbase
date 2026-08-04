const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const {
  HEAVY_VERIFY_LOCK_DIR,
  LOCAL_VERIFY_CONFIG_FILE,
  acquireHeavyVerifyLock,
  isCiEnvironment,
  loadVerifyRuntimeConfig,
  main,
  readHeavyVerifyLockOwner,
  withHeavyVerifyLock,
} = require("../verify-runtime.js");

function createRepoRoot() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "oneflowbase-verify-runtime-"));
}

test("loadVerifyRuntimeConfig returns defaults when local config is absent", () => {
  const repoRoot = createRepoRoot();

  const config = loadVerifyRuntimeConfig({
    repoRoot,
    env: {},
    availableParallelism: 8,
  });

  assert.deepEqual(config, {
    backend: {
      cargoJobs: 8,
      cargoTestThreads: 8,
    },
    frontend: {
      turboConcurrency: 8,
      vitestMaxWorkers: 6,
    },
    locks: {
      waitTimeoutMinutes: 30,
      waitTimeoutMs: 30 * 60 * 1000,
      pollIntervalMs: 5000,
    },
  });
});

test("frontend defaults reserve local CPU headroom on large machines", () => {
  const repoRoot = createRepoRoot();

  const config = loadVerifyRuntimeConfig({
    repoRoot,
    env: {},
    availableParallelism: 16,
  });

  assert.equal(config.frontend.vitestMaxWorkers, 12);
  assert.equal(config.frontend.turboConcurrency, 16);
});

test("cargo-jobs prints all machine parallelism by default", () => {
  const repoRoot = createRepoRoot();
  let stdout = "";

  const status = main(["cargo-jobs"], {
    repoRoot,
    env: {},
    availableParallelism: 10,
    writeStdout: (text) => {
      stdout += text;
    },
  });

  assert.equal(status, 0);
  assert.equal(stdout, "10\n");
});

test("loadVerifyRuntimeConfig applies local overrides when config file exists", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        backend: {
          cargoJobs: 3,
          cargoTestThreads: 2,
        },
        frontend: {
          turboConcurrency: 2,
          vitestMaxWorkers: 3,
        },
        locks: {
          waitTimeoutMinutes: 12,
          pollIntervalMs: 1500,
        },
      },
      null,
      2,
    ),
  );

  const config = loadVerifyRuntimeConfig({
    repoRoot,
    env: {},
    availableParallelism: 8,
  });

  assert.deepEqual(config, {
    backend: {
      cargoJobs: 3,
      cargoTestThreads: 2,
    },
    frontend: {
      turboConcurrency: 2,
      vitestMaxWorkers: 3,
    },
    locks: {
      waitTimeoutMinutes: 12,
      waitTimeoutMs: 12 * 60 * 1000,
      pollIntervalMs: 1500,
    },
  });
});

test("loadVerifyRuntimeConfig rejects unknown top-level keys", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        backend: {
          cargoJobs: 3,
          cargoTestThreads: 2,
        },
        locks: {
          waitTimeoutMinutes: 12,
          pollIntervalMs: 1500,
        },
        extra: true,
      },
      null,
      2,
    ),
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 8,
      }),
    /Unknown verify runtime config key: extra/,
  );
});

test("loadVerifyRuntimeConfig rejects unknown backend keys", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        backend: {
          cargoJobs: 3,
          cargoTestThreads: 2,
          unexpected: true,
        },
      },
      null,
      2,
    ),
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 8,
      }),
    /Unknown backend key: unexpected/,
  );
});

test("loadVerifyRuntimeConfig rejects unknown frontend keys", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        frontend: {
          turboConcurrency: 2,
          unexpected: true,
        },
      },
      null,
      2,
    ),
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 8,
      }),
    /Unknown frontend key: unexpected/,
  );
});

test("loadVerifyRuntimeConfig rejects unknown locks keys", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        locks: {
          waitTimeoutMinutes: 12,
          pollIntervalMs: 1500,
          unexpected: true,
        },
      },
      null,
      2,
    ),
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 8,
      }),
    /Unknown locks key: unexpected/,
  );
});

test("loadVerifyRuntimeConfig merges backend and lock overrides field by field", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        backend: {
          cargoJobs: 3,
        },
        frontend: {
          vitestMaxWorkers: 2,
        },
        locks: {
          waitTimeoutMinutes: 12,
        },
      },
      null,
      2,
    ),
  );

  const config = loadVerifyRuntimeConfig({
    repoRoot,
    env: {},
    availableParallelism: 8,
  });

  assert.deepEqual(config, {
    backend: {
      cargoJobs: 3,
      cargoTestThreads: 8,
    },
    frontend: {
      turboConcurrency: 8,
      vitestMaxWorkers: 2,
    },
    locks: {
      waitTimeoutMinutes: 12,
      waitTimeoutMs: 12 * 60 * 1000,
      pollIntervalMs: 5000,
    },
  });
});

test("loadVerifyRuntimeConfig uses full runner capacity and ignores local config in CI", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        backend: {
          cargoJobs: 1,
          cargoTestThreads: 1,
        },
        locks: {
          waitTimeoutMinutes: 1,
          pollIntervalMs: 1,
        },
      },
      null,
      2,
    ),
  );

  const config = loadVerifyRuntimeConfig({
    repoRoot,
    env: { CI: "true" },
    availableParallelism: 10,
  });

  assert.deepEqual(config, {
    backend: {
      cargoJobs: 10,
      cargoTestThreads: 10,
    },
    frontend: {
      turboConcurrency: 10,
      vitestMaxWorkers: 10,
    },
    locks: {
      waitTimeoutMinutes: 30,
      waitTimeoutMs: 30 * 60 * 1000,
      pollIntervalMs: 5000,
    },
  });
});

test("isCiEnvironment accepts common truthy CI variants", () => {
  const cases = [
    [{ CI: "true" }, true],
    [{ CI: "TRUE" }, true],
    [{ CI: "1" }, true],
    [{ GITHUB_ACTIONS: "true" }, true],
    [{ GITHUB_ACTIONS: "TRUE" }, true],
    [{ GITHUB_ACTIONS: "1" }, true],
    [{ CI: "false", GITHUB_ACTIONS: "true" }, true],
    [{ CI: "0", GITHUB_ACTIONS: "0" }, false],
    [{}, false],
  ];

  for (const [env, expected] of cases) {
    assert.equal(isCiEnvironment(env), expected);
  }
});

test("loadVerifyRuntimeConfig rejects invalid backend cargoJobs", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        backend: {
          cargoJobs: 0,
        },
      },
      null,
      2,
    ),
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 4,
      }),
    /must be a positive integer/i,
  );
});

test("loadVerifyRuntimeConfig rejects invalid backend cargoTestThreads", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        backend: {
          cargoTestThreads: 0,
        },
      },
      null,
      2,
    ),
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 4,
      }),
    /backend\.cargoTestThreads must be a positive integer/,
  );
});

test("loadVerifyRuntimeConfig rejects invalid frontend turboConcurrency", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        frontend: {
          turboConcurrency: 0,
        },
      },
      null,
      2,
    ),
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 4,
      }),
    /frontend\.turboConcurrency must be a positive integer/,
  );
});

test("loadVerifyRuntimeConfig rejects frontend vitestMaxWorkers above available parallelism", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        frontend: {
          vitestMaxWorkers: 5,
        },
      },
      null,
      2,
    ),
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 4,
      }),
    /frontend\.vitestMaxWorkers must not exceed availableParallelism/,
  );
});

test("loadVerifyRuntimeConfig rejects invalid locks waitTimeoutMinutes", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        locks: {
          waitTimeoutMinutes: 0,
        },
      },
      null,
      2,
    ),
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 4,
      }),
    /locks\.waitTimeoutMinutes must be a positive integer/,
  );
});

test("loadVerifyRuntimeConfig rejects invalid locks pollIntervalMs", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        locks: {
          pollIntervalMs: 0,
        },
      },
      null,
      2,
    ),
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 4,
      }),
    /locks\.pollIntervalMs must be a positive integer/,
  );
});

test("loadVerifyRuntimeConfig rejects backend cargoTestThreads above available parallelism", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        backend: {
          cargoTestThreads: 5,
        },
      },
      null,
      2,
    ),
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 4,
      }),
    /backend\.cargoTestThreads must not exceed availableParallelism/,
  );
});

test("loadVerifyRuntimeConfig rejects missing repoRoot", () => {
  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        env: {},
        availableParallelism: 4,
      }),
    /repoRoot must be a non-empty string/,
  );
});

test("loadVerifyRuntimeConfig rejects empty repoRoot", () => {
  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot: "",
        env: {},
        availableParallelism: 4,
      }),
    /repoRoot must be a non-empty string/,
  );
});

test("loadVerifyRuntimeConfig rejects non-object root config values", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE), "null");

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 4,
      }),
    /verify runtime config root must be a plain object/,
  );
});

test("loadVerifyRuntimeConfig rejects array root config values", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE), "[]");

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 4,
      }),
    /verify runtime config root must be a plain object/,
  );
});

test("loadVerifyRuntimeConfig rejects non-object backend config values", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        backend: [],
      },
      null,
      2,
    ),
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 4,
      }),
    /backend must be a plain object/,
  );
});

test("loadVerifyRuntimeConfig rejects non-object frontend config values", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        frontend: [],
      },
      null,
      2,
    ),
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 4,
      }),
    /frontend must be a plain object/,
  );
});

test("loadVerifyRuntimeConfig rejects non-object locks config values", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    JSON.stringify(
      {
        locks: [],
      },
      null,
      2,
    ),
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 4,
      }),
    /locks must be a plain object/,
  );
});

test("loadVerifyRuntimeConfig rejects invalid JSON", () => {
  const repoRoot = createRepoRoot();

  fs.writeFileSync(
    path.join(repoRoot, LOCAL_VERIFY_CONFIG_FILE),
    "{ not valid json",
  );

  assert.throws(
    () =>
      loadVerifyRuntimeConfig({
        repoRoot,
        env: {},
        availableParallelism: 4,
      }),
    /Unexpected token|JSON/i,
  );
});

test("acquireHeavyVerifyLock writes owner.json with the expected metadata", async () => {
  const repoRoot = createRepoRoot();
  const output = [];

  const lock = await acquireHeavyVerifyLock({
    repoRoot,
    env: {},
    scope: "verify-backend",
    command: "node scripts/node/verify-backend.js",
    now: () => new Date("2026-04-21T12:10:00.000Z"),
    hostname: "devbox",
    processId: 321,
    sleepImpl: async () => {},
    isProcessAliveImpl: () => true,
    writeStdout: (text) => output.push(text),
  });

  const owner = readHeavyVerifyLockOwner({ repoRoot });

  assert.equal(owner.pid, 321);
  assert.equal(owner.scope, "verify-backend");
  assert.equal(owner.command, "node scripts/node/verify-backend.js");
  assert.equal(owner.cwd, repoRoot);
  assert.equal(owner.startedAt, "2026-04-21T12:10:00.000Z");
  assert.equal(owner.hostname, "devbox");
  assert.equal(typeof owner.token, "string");
  assert.equal(owner.token.length > 0, true);

  lock.release();
  assert.equal(
    fs.existsSync(path.join(repoRoot, HEAVY_VERIFY_LOCK_DIR)),
    false,
  );
  assert.match(
    output.join(""),
    /\[1flowbase-verify-lock\] acquired: scope=verify-backend pid=321/u,
  );
  assert.match(
    output.join(""),
    /\[1flowbase-verify-lock\] released: scope=verify-backend pid=321/u,
  );
});

test("acquireHeavyVerifyLock waits for a live owner and times out", async () => {
  const repoRoot = createRepoRoot();
  const output = [];

  fs.mkdirSync(path.join(repoRoot, HEAVY_VERIFY_LOCK_DIR), { recursive: true });
  fs.writeFileSync(
    path.join(repoRoot, HEAVY_VERIFY_LOCK_DIR, "owner.json"),
    JSON.stringify(
      {
        token: "existing-token",
        pid: 999,
        scope: "verify-repo",
        command: "node scripts/node/verify-repo.js",
        cwd: repoRoot,
        startedAt: "2026-04-21T12:00:00.000Z",
        hostname: "devbox",
      },
      null,
      2,
    ),
  );

  const nowValues = [
    new Date("2026-04-21T12:00:00.000Z"),
    new Date("2026-04-21T12:00:30.000Z"),
    new Date("2026-04-21T12:01:01.000Z"),
  ];

  await assert.rejects(
    acquireHeavyVerifyLock({
      repoRoot,
      env: {},
      scope: "verify-backend",
      command: "node scripts/node/verify-backend.js",
      runtimeConfig: {
        backend: {
          cargoJobs: 1,
          cargoTestThreads: 1,
        },
        locks: {
          waitTimeoutMinutes: 1,
          waitTimeoutMs: 60_000,
          pollIntervalMs: 1000,
        },
      },
      writeStdout: (text) => output.push(text),
      now: () => nowValues.shift() ?? new Date("2026-04-21T12:01:01.000Z"),
      sleepImpl: async () => {},
      isProcessAliveImpl: () => true,
      processId: 654,
    }),
    /timeout waiting for heavy-verify lock/u,
  );

  assert.match(
    output.join(""),
    /\[1flowbase-verify-lock\] busy: scope=verify-repo pid=999/u,
  );
  assert.match(output.join(""), /\[1flowbase-verify-lock\] waiting.../u);
  assert.match(
    output.join(""),
    /\[1flowbase-verify-lock\] timeout waiting for heavy-verify lock:/u,
  );
});

test("acquireHeavyVerifyLock cleans a stale owner before retrying", async () => {
  const repoRoot = createRepoRoot();
  const output = [];

  fs.mkdirSync(path.join(repoRoot, HEAVY_VERIFY_LOCK_DIR), { recursive: true });
  fs.writeFileSync(
    path.join(repoRoot, HEAVY_VERIFY_LOCK_DIR, "owner.json"),
    JSON.stringify(
      {
        token: "stale-token",
        pid: 555,
        scope: "verify-ci",
        command: "node scripts/node/verify-ci.js",
        cwd: repoRoot,
        startedAt: "2026-04-21T11:00:00.000Z",
        hostname: "devbox",
      },
      null,
      2,
    ),
  );

  const lock = await acquireHeavyVerifyLock({
    repoRoot,
    env: {},
    scope: "verify-backend",
    command: "node scripts/node/verify-backend.js",
    writeStdout: (text) => output.push(text),
    isProcessAliveImpl: () => false,
    sleepImpl: async () => {},
    processId: 777,
  });

  assert.match(
    output.join(""),
    /\[1flowbase-verify-lock\] stale lock detected, cleaning/u,
  );
  assert.equal(readHeavyVerifyLockOwner({ repoRoot }).pid, 777);
  lock.release();
});

test("acquireHeavyVerifyLock cleans a damaged owner before retrying", async () => {
  const repoRoot = createRepoRoot();
  const output = [];

  fs.mkdirSync(path.join(repoRoot, HEAVY_VERIFY_LOCK_DIR), { recursive: true });
  fs.writeFileSync(
    path.join(repoRoot, HEAVY_VERIFY_LOCK_DIR, "owner.json"),
    "{ not valid json",
  );

  const lock = await acquireHeavyVerifyLock({
    repoRoot,
    env: {},
    scope: "verify-backend",
    command: "node scripts/node/verify-backend.js",
    writeStdout: (text) => output.push(text),
    isProcessAliveImpl: () => true,
    sleepImpl: async () => {},
    processId: 778,
  });

  assert.match(
    output.join(""),
    /\[1flowbase-verify-lock\] stale lock detected, cleaning/u,
  );
  assert.equal(readHeavyVerifyLockOwner({ repoRoot }).pid, 778);
  lock.release();
});

test("acquireHeavyVerifyLock cleans a missing owner.json before retrying", async () => {
  const repoRoot = createRepoRoot();
  const output = [];

  fs.mkdirSync(path.join(repoRoot, HEAVY_VERIFY_LOCK_DIR), { recursive: true });

  const lock = await acquireHeavyVerifyLock({
    repoRoot,
    env: {},
    scope: "verify-backend",
    command: "node scripts/node/verify-backend.js",
    writeStdout: (text) => output.push(text),
    sleepImpl: async () => {},
    isProcessAliveImpl: () => true,
    processId: 779,
  });

  assert.match(
    output.join(""),
    /\[1flowbase-verify-lock\] stale lock detected, cleaning/u,
  );
  assert.equal(readHeavyVerifyLockOwner({ repoRoot }).pid, 779);
  lock.release();
});

test("acquireHeavyVerifyLock publishes the lock atomically before exposing lockDir", async () => {
  const repoRoot = createRepoRoot();
  const output = [];
  const publishEntered = createDeferred();
  const releasePublish = createDeferred();
  const lockDir = path.join(repoRoot, HEAVY_VERIFY_LOCK_DIR);
  const nowValues = [
    new Date("2026-04-21T12:00:00.000Z"),
    new Date("2026-04-21T12:00:01.000Z"),
    new Date("2026-04-21T12:00:02.000Z"),
  ];

  const acquirePromise = acquireHeavyVerifyLock({
    repoRoot,
    env: {},
    scope: "verify-backend",
    command: "node scripts/node/verify-backend.js",
    runtimeConfig: {
      backend: {
        cargoJobs: 1,
        cargoTestThreads: 1,
      },
      locks: {
        waitTimeoutMinutes: 1,
        waitTimeoutMs: 1000,
        pollIntervalMs: 1,
      },
    },
    writeStdout: (text) => output.push(text),
    beforeOwnerPublishImpl: async ({ stagingDir }) => {
      assert.equal(fs.existsSync(lockDir), false);
      assert.equal(fs.existsSync(path.join(stagingDir, "owner.json")), true);
      publishEntered.resolve();
      await releasePublish.promise;
    },
    now: () => nowValues.shift() ?? new Date("2026-04-21T12:00:02.000Z"),
    sleepImpl: async () => {},
    isProcessAliveImpl: () => true,
    processId: 901,
  });

  await publishEntered.promise;
  assert.equal(fs.existsSync(lockDir), false);

  fs.mkdirSync(lockDir, { recursive: true });
  fs.writeFileSync(
    path.join(lockDir, "owner.json"),
    JSON.stringify(
      {
        token: "competing-token",
        pid: 999,
        scope: "verify-repo",
        command: "node scripts/node/verify-repo.js",
        cwd: repoRoot,
        startedAt: "2026-04-21T11:59:59.000Z",
        hostname: "devbox",
      },
      null,
      2,
    ),
  );

  releasePublish.resolve();

  await assert.rejects(
    acquirePromise,
    /timeout waiting for heavy-verify lock/u,
  );

  assert.match(
    output.join(""),
    /\[1flowbase-verify-lock\] busy: scope=verify-repo pid=999/u,
  );
  assert.match(
    output.join(""),
    /\[1flowbase-verify-lock\] timeout waiting for heavy-verify lock:/u,
  );
  assert.equal(readHeavyVerifyLockOwner({ repoRoot }).pid, 999);
});

test("acquireHeavyVerifyLock treats matching token as a reentrant owner", async () => {
  const repoRoot = createRepoRoot();
  const token = "11111111-2222-4333-8444-555555555555";

  fs.mkdirSync(path.join(repoRoot, HEAVY_VERIFY_LOCK_DIR), { recursive: true });
  fs.writeFileSync(
    path.join(repoRoot, HEAVY_VERIFY_LOCK_DIR, "owner.json"),
    JSON.stringify(
      {
        token,
        pid: 444,
        scope: "verify-repo",
        command: "node scripts/node/verify-repo.js",
        cwd: repoRoot,
        startedAt: "2026-04-21T12:00:00.000Z",
        hostname: "devbox",
      },
      null,
      2,
    ),
  );

  const lock = await acquireHeavyVerifyLock({
    repoRoot,
    env: { ONEFLOWBASE_VERIFY_LOCK_TOKEN: token },
    scope: "verify-backend",
    command: "node scripts/node/verify-backend.js",
    processId: 444,
    sleepImpl: async () => {},
    isProcessAliveImpl: () => true,
  });

  assert.equal(lock.token, token);
  assert.equal(lock.reentrant, true);
});

test("withHeavyVerifyLock releases the lock when cleanup events fire", async () => {
  const cleanupEvents = [
    "SIGINT",
    "SIGTERM",
    "uncaughtException",
    "unhandledRejection",
  ];

  for (const eventName of cleanupEvents) {
    const repoRoot = createRepoRoot();
    const listeners = new Map();
    const output = [];

    await withHeavyVerifyLock(
      {
        repoRoot,
        env: {},
        scope: "verify-backend",
        command: "node scripts/node/verify-backend.js",
        runtimeConfig: {
          backend: {
            cargoJobs: 1,
            cargoTestThreads: 1,
          },
          locks: {
            waitTimeoutMinutes: 1,
            waitTimeoutMs: 60_000,
            pollIntervalMs: 1000,
          },
        },
        writeStdout: (text) => output.push(text),
        processEmitter: {
          once(name, handler) {
            listeners.set(name, handler);
          },
          removeListener(name, handler) {
            if (listeners.get(name) === handler) {
              listeners.delete(name);
            }
          },
        },
        sleepImpl: async () => {},
        isProcessAliveImpl: () => true,
        processId: 888,
      },
      async () => {
        for (const name of cleanupEvents) {
          assert.equal(typeof listeners.get(name), "function");
        }

        listeners.get(eventName)();
        assert.equal(
          fs.existsSync(path.join(repoRoot, HEAVY_VERIFY_LOCK_DIR)),
          false,
        );
        assert.match(
          output.join(""),
          /\[1flowbase-verify-lock\] released: scope=verify-backend pid=888/u,
        );
        return 0;
      },
    );
  }
});

test("withHeavyVerifyLock forwards fatal cleanup events after releasing the lock", async () => {
  const repoRoot = createRepoRoot();
  const listeners = new Map();
  const forwarded = [];
  const error = new Error("boom");

  await withHeavyVerifyLock(
    {
      repoRoot,
      env: {},
      scope: "verify-backend",
      command: "node scripts/node/verify-backend.js",
      runtimeConfig: {
        backend: {
          cargoJobs: 1,
          cargoTestThreads: 1,
        },
        locks: {
          waitTimeoutMinutes: 1,
          waitTimeoutMs: 60_000,
          pollIntervalMs: 1000,
        },
      },
      processEmitter: {
        once(name, handler) {
          listeners.set(name, handler);
        },
        removeListener(name, handler) {
          if (listeners.get(name) === handler) {
            listeners.delete(name);
          }
        },
      },
      forwardFatalCleanupEvent(eventName, payload) {
        forwarded.push({ eventName, payload });
      },
      sleepImpl: async () => {},
      isProcessAliveImpl: () => true,
      processId: 889,
    },
    async () => {
      listeners.get("uncaughtException")(error);
      assert.equal(
        fs.existsSync(path.join(repoRoot, HEAVY_VERIFY_LOCK_DIR)),
        false,
      );
      assert.deepEqual(forwarded, [
        { eventName: "uncaughtException", payload: error },
      ]);
      return 0;
    },
  );
});

function createDeferred() {
  let resolve;
  let reject;
  const promise = new Promise((res, rej) => {
    resolve = res;
    reject = rej;
  });

  return { promise, resolve, reject };
}
