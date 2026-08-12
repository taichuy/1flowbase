const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");

const {
  assertSafeArchiveEntries,
  detectTarget,
  installPinnedToolchain,
  resolvePostgresToolchain,
  shouldResolveForAction,
} = require("../resolver.js");

function tempRepo() {
  return fs.mkdtempSync(
    path.join(os.tmpdir(), "oneflowbase-postgres-toolchain-"),
  );
}

function versionSpawn(command, args, options) {
  if (command === "tar") return spawnSync(command, args, options);
  return { status: 0, stdout: "pg_dump (PostgreSQL) 18.4\n", stderr: "" };
}

test("maps supported development targets and rejects unsupported combinations", () => {
  assert.equal(
    detectTarget({ platform: "linux", arch: "x64", libc: "gnu" }),
    "x86_64-unknown-linux-gnu",
  );
  assert.equal(
    detectTarget({ platform: "linux", arch: "arm64", libc: "musl" }),
    "aarch64-unknown-linux-musl",
  );
  assert.equal(
    detectTarget({ platform: "darwin", arch: "arm64" }),
    "aarch64-apple-darwin",
  );
  assert.equal(
    detectTarget({ platform: "win32", arch: "x64" }),
    "x86_64-pc-windows-msvc",
  );
  assert.equal(detectTarget({ platform: "win32", arch: "arm64" }), null);
});

test("only resolves toolchains for backend start-like actions", () => {
  assert.equal(shouldResolveForAction("restart", ["api-server"]), true);
  assert.equal(shouldResolveForAction("start", ["web"]), false);
  assert.equal(shouldResolveForAction("stop", ["api-server"]), false);
});

test("explicit tool paths take priority without a download", async () => {
  const repoRoot = tempRepo();
  const pgDumpPath = path.join(repoRoot, "pg_dump");
  const pgRestorePath = path.join(repoRoot, "pg_restore");
  fs.writeFileSync(pgDumpPath, "fixture");
  fs.writeFileSync(pgRestorePath, "fixture");
  const result = await resolvePostgresToolchain({
    repoRoot,
    sourceEnv: {
      API_POSTGRES_PG_DUMP_PATH: pgDumpPath,
      API_POSTGRES_PG_RESTORE_PATH: pgRestorePath,
    },
    platform: "linux",
    arch: "x64",
    libc: "gnu",
    spawnImpl: versionSpawn,
    fetchImpl: async () => {
      throw new Error("download must not run");
    },
  });
  assert.equal(result.source, "explicit");
  assert.equal(result.pgDumpPath, pgDumpPath);
  fs.rmSync(repoRoot, { recursive: true, force: true });
});

test("partial explicit configuration warns and does not fall through to download", async () => {
  const messages = [];
  const result = await resolvePostgresToolchain({
    repoRoot: tempRepo(),
    sourceEnv: { API_POSTGRES_PG_DUMP_PATH: "/fixture/pg_dump" },
    logImpl: (message) => messages.push(message),
    fetchImpl: async () => {
      throw new Error("download must not run");
    },
  });
  assert.equal(result, null);
  assert.match(messages[0], /configured as a pair/u);
  assert.match(messages[0], /API startup will continue/u);
});

test("archive traversal and absolute paths are rejected before extraction", () => {
  assert.throws(() => assertSafeArchiveEntries("../escape"), /path traversal/u);
  assert.throws(
    () => assertSafeArchiveEntries("/absolute/file"),
    /absolute path/u,
  );
  assert.doesNotThrow(() =>
    assertSafeArchiveEntries("postgresql/bin/pg_dump\npostgresql/lib/libpq.so"),
  );
});

test("pinned install verifies checksum, writes an atomic receipt, and reuses cache", async () => {
  const repoRoot = tempRepo();
  const fixtureRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "oneflowbase-postgres-archive-"),
  );
  const archiveRoot = path.join(fixtureRoot, "postgresql-fixture");
  fs.mkdirSync(path.join(archiveRoot, "bin"), { recursive: true });
  fs.writeFileSync(path.join(archiveRoot, "bin", "pg_dump"), "fixture");
  fs.writeFileSync(path.join(archiveRoot, "bin", "pg_restore"), "fixture");
  const archivePath = path.join(fixtureRoot, "fixture.tar.gz");
  const packed = spawnSync(
    "tar",
    ["-czf", archivePath, "-C", fixtureRoot, "postgresql-fixture"],
    { encoding: "utf8" },
  );
  assert.equal(packed.status, 0, packed.stderr);
  const bytes = fs.readFileSync(archivePath);
  const sha256 = crypto.createHash("sha256").update(bytes).digest("hex");
  const manifest = {
    postgresVersion: "18.4.0-test",
    expectedMajor: 18,
    targets: {
      "x86_64-unknown-linux-gnu": {
        url: "https://example.invalid/fixture.tar.gz",
        sha256,
      },
    },
  };
  let downloads = 0;
  const options = {
    repoRoot,
    target: "x86_64-unknown-linux-gnu",
    platform: "linux",
    manifest,
    spawnImpl: versionSpawn,
    fetchImpl: async () => {
      downloads += 1;
      await new Promise((resolve) => setTimeout(resolve, 30));
      return new Response(bytes);
    },
  };
  const installs = await Promise.all([
    installPinnedToolchain(options),
    installPinnedToolchain(options),
  ]);
  assert.deepEqual(installs.map((result) => result.source).sort(), [
    "cache",
    "download",
  ]);
  assert.equal(downloads, 1);
  assert.equal(
    JSON.parse(
      fs.readFileSync(
        path.join(path.dirname(installs[0].pgDumpPath), "..", "receipt.json"),
        "utf8",
      ),
    ).sha256,
    sha256,
  );
  const cached = await installPinnedToolchain(options);
  assert.equal(cached.source, "cache");
  assert.equal(downloads, 1);
  fs.rmSync(repoRoot, { recursive: true, force: true });
  fs.rmSync(fixtureRoot, { recursive: true, force: true });
});

test("checksum failure leaves no receipt or staging directory and only disables backup", async () => {
  const repoRoot = tempRepo();
  const target = "x86_64-unknown-linux-gnu";
  const manifest = {
    postgresVersion: "18.4.0-test",
    expectedMajor: 18,
    targets: {
      [target]: {
        url: "https://example.invalid/bad.tar.gz",
        sha256: "0".repeat(64),
      },
    },
  };
  const messages = [];
  const result = await resolvePostgresToolchain({
    repoRoot,
    sourceEnv: { PATH: "" },
    platform: "linux",
    arch: "x64",
    libc: "gnu",
    manifest,
    fetchImpl: async () => new Response("corrupt"),
    spawnImpl: versionSpawn,
    logImpl: (message) => messages.push(message),
  });
  assert.equal(result, null);
  assert.match(messages[0], /checksum mismatch/u);
  const versionRoot = path.join(
    repoRoot,
    "tmp",
    "toolchains",
    "postgresql",
    manifest.postgresVersion,
  );
  assert.equal(
    fs.existsSync(path.join(versionRoot, target, "receipt.json")),
    false,
  );
  assert.deepEqual(fs.readdirSync(versionRoot), []);
  fs.rmSync(repoRoot, { recursive: true, force: true });
});
