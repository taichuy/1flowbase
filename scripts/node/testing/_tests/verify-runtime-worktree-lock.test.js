const test = require("node:test");
const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const {
  HEAVY_VERIFY_LOCK_DIR,
  acquireHeavyVerifyLock,
  readHeavyVerifyLockOwner,
  resolveHeavyVerifyLockPath,
} = require("../verify-runtime.js");

test("linked git worktrees share a single machine-wide heavy verify lock via git-common-dir while non-git fixtures stay local", async (t) => {
  const sandbox = fs.mkdtempSync(path.join(os.tmpdir(), "oneflowbase-worktree-lock-"));
  t.after(() => fs.rmSync(sandbox, { recursive: true, force: true }));

  const nonGitFixture = path.join(sandbox, "plain-fixture");
  fs.mkdirSync(nonGitFixture, { recursive: true });
  assert.equal(
    resolveHeavyVerifyLockPath(nonGitFixture),
    path.join(nonGitFixture, HEAVY_VERIFY_LOCK_DIR),
  );

  const mainRepo = path.join(sandbox, "main-repo");
  const worktreeA = path.join(sandbox, "wt-a");
  const worktreeB = path.join(sandbox, "wt-b");
  fs.mkdirSync(mainRepo, { recursive: true });

  const git = (cwd, args) => {
    const res = spawnSync("git", args, { cwd, encoding: "utf8" });
    assert.equal(res.status, 0, res.stderr || res.error?.message);
  };
  git(mainRepo, ["init"]);
  git(mainRepo, ["config", "user.email", "test@example.com"]);
  git(mainRepo, ["config", "user.name", "Test"]);
  git(mainRepo, ["commit", "--allow-empty", "-m", "init"]);
  git(mainRepo, ["worktree", "add", "-b", "wt-a", worktreeA]);
  git(mainRepo, ["worktree", "add", "-b", "wt-b", worktreeB]);

  const expectedSharedLock = path.join(
    fs.realpathSync(path.join(mainRepo, ".git")),
    "test-governance",
    "locks",
    "heavy-verify",
  );
  assert.equal(resolveHeavyVerifyLockPath(mainRepo), expectedSharedLock);
  assert.equal(resolveHeavyVerifyLockPath(worktreeA), expectedSharedLock);
  assert.equal(resolveHeavyVerifyLockPath(worktreeB), expectedSharedLock);

  const runtimeConfig = {
    backend: { cargoJobs: 2, cargoTestThreads: 2, incremental: false },
    locks: { waitTimeoutMinutes: 1, waitTimeoutMs: 50, pollIntervalMs: 10 },
  };

  const lockA = await acquireHeavyVerifyLock({
    repoRoot: worktreeA,
    env: {},
    scope: "verify-backend-wt-a",
    command: "node scripts/node/verify-backend.js",
    runtimeConfig,
    writeStdout() {},
    processId: 4101,
    isProcessAliveImpl: (pid) => pid === 4101,
  });

  try {
    const visibleFromB = readHeavyVerifyLockOwner({ repoRoot: worktreeB });
    assert.equal(visibleFromB?.pid, 4101);
    assert.equal(visibleFromB?.cwd, worktreeA);

    let nowMs = 1000;
    await assert.rejects(
      () =>
        acquireHeavyVerifyLock({
          repoRoot: worktreeB,
          env: {},
          scope: "verify-backend-wt-b",
          command: "node scripts/node/verify-backend.js",
          runtimeConfig,
          writeStdout() {},
          processId: 4102,
          isProcessAliveImpl: () => true,
          now: () => new Date(nowMs),
          sleepImpl: async (ms) => {
            nowMs += ms;
          },
        }),
      /timeout waiting for heavy-verify lock/u,
    );
  } finally {
    lockA.release();
  }

  assert.equal(readHeavyVerifyLockOwner({ repoRoot: worktreeB }), null);
});
