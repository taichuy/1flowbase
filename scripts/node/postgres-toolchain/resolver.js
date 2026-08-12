const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const { pipeline } = require("node:stream/promises");
const { Readable } = require("node:stream");
const { spawnSync } = require("node:child_process");

const lock = require("./lock.json");
const EXPLICIT_DUMP_ENV = "API_POSTGRES_PG_DUMP_PATH";
const EXPLICIT_RESTORE_ENV = "API_POSTGRES_PG_RESTORE_PATH";
const START_ACTIONS = new Set(["start", "ensure", "restart"]);

function detectTarget({
  platform = process.platform,
  arch = process.arch,
  libc,
} = {}) {
  const cpu = arch === "x64" ? "x86_64" : arch === "arm64" ? "aarch64" : null;
  if (!cpu) return null;
  if (platform === "darwin") return `${cpu}-apple-darwin`;
  if (platform === "win32")
    return cpu === "x86_64" ? `${cpu}-pc-windows-msvc` : null;
  if (platform !== "linux") return null;
  const resolvedLibc =
    libc ||
    (process.report?.getReport()?.header?.glibcVersionRuntime ? "gnu" : "musl");
  return `${cpu}-unknown-linux-${resolvedLibc}`;
}

function executableNames(platform = process.platform) {
  const suffix = platform === "win32" ? ".exe" : "";
  return { pgDump: `pg_dump${suffix}`, pgRestore: `pg_restore${suffix}` };
}

function toolPaths(root, platform = process.platform) {
  const names = executableNames(platform);
  return {
    pgDumpPath: path.join(root, "bin", names.pgDump),
    pgRestorePath: path.join(root, "bin", names.pgRestore),
  };
}

function commandVersion(command, spawnImpl = spawnSync) {
  if (!command || !fs.existsSync(command)) return null;
  const result = spawnImpl(command, ["--version"], {
    encoding: "utf8",
    env: { ...process.env, LC_ALL: "C" },
    windowsHide: true,
  });
  if (result.status !== 0) return null;
  const match = String(result.stdout || "").match(
    /PostgreSQL\)?\s+(\d+)(?:\.(\d+))?/u,
  );
  return match
    ? { major: Number(match[1]), output: String(result.stdout).trim() }
    : null;
}

function validatePair(
  pair,
  expectedMajor = lock.expectedMajor,
  spawnImpl = spawnSync,
) {
  const dump = commandVersion(pair.pgDumpPath, spawnImpl);
  const restore = commandVersion(pair.pgRestorePath, spawnImpl);
  return Boolean(
    dump &&
    restore &&
    dump.major === expectedMajor &&
    restore.major === expectedMajor,
  );
}

function pathCommand(
  name,
  sourceEnv = process.env,
  platform = process.platform,
) {
  const suffixes = platform === "win32" ? [".exe", ".cmd", ".bat", ""] : [""];
  for (const directory of String(sourceEnv.PATH || "")
    .split(path.delimiter)
    .filter(Boolean)) {
    for (const suffix of suffixes) {
      const candidate = path.join(directory, `${name}${suffix}`);
      if (fs.existsSync(candidate)) return candidate;
    }
  }
  return null;
}

function assertSafeArchiveEntries(output) {
  const entries = String(output).split(/\r?\n/u).filter(Boolean);
  if (entries.length === 0) throw new Error("archive is empty");
  for (const entry of entries) {
    const normalized = entry.replaceAll("\\", "/");
    if (normalized.startsWith("/") || /^[A-Za-z]:\//u.test(normalized)) {
      throw new Error(`archive contains an absolute path: ${entry}`);
    }
    if (normalized.split("/").includes("..")) {
      throw new Error(`archive contains path traversal: ${entry}`);
    }
  }
}

function runTar(archivePath, stagingPath, spawnImpl = spawnSync) {
  const listing = spawnImpl("tar", ["-tzf", archivePath], {
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    windowsHide: true,
  });
  if (listing.status !== 0)
    throw new Error(
      `archive listing failed: ${String(listing.stderr || "").trim()}`,
    );
  assertSafeArchiveEntries(listing.stdout);
  const extracted = spawnImpl(
    "tar",
    ["-xzf", archivePath, "--strip-components=1", "-C", stagingPath],
    {
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
      windowsHide: true,
    },
  );
  if (extracted.status !== 0)
    throw new Error(
      `archive extraction failed: ${String(extracted.stderr || "").trim()}`,
    );
}

async function sha256File(filePath) {
  const hash = crypto.createHash("sha256");
  await pipeline(fs.createReadStream(filePath), hash);
  return hash.digest("hex");
}

function sha256FileSync(filePath) {
  return crypto
    .createHash("sha256")
    .update(fs.readFileSync(filePath))
    .digest("hex");
}

async function downloadFile(url, destination, fetchImpl = globalThis.fetch) {
  const response = await fetchImpl(url, {
    redirect: "follow",
    signal: AbortSignal.timeout(45_000),
  });
  if (!response.ok || !response.body)
    throw new Error(`download returned HTTP ${response.status}`);
  await pipeline(
    Readable.fromWeb(response.body),
    fs.createWriteStream(destination, { flags: "wx" }),
  );
}

async function acquireLock(
  lockPath,
  { timeoutMs = 30_000, pollMs = 100, staleMs = 5 * 60_000 } = {},
) {
  const started = Date.now();
  while (true) {
    try {
      fs.mkdirSync(lockPath);
      return () => fs.rmSync(lockPath, { recursive: true, force: true });
    } catch (error) {
      if (error.code !== "EEXIST") throw error;
      try {
        if (Date.now() - fs.statSync(lockPath).mtimeMs > staleMs) {
          fs.rmSync(lockPath, { recursive: true, force: true });
          continue;
        }
      } catch (statError) {
        if (statError.code !== "ENOENT") throw statError;
        continue;
      }
      if (Date.now() - started >= timeoutMs)
        throw new Error("timed out waiting for toolchain install lock");
      await new Promise((resolve) => setTimeout(resolve, pollMs));
    }
  }
}

function readValidCache(
  installRoot,
  target,
  platform,
  spawnImpl,
  manifest = lock,
) {
  const receiptPath = path.join(installRoot, "receipt.json");
  if (!fs.existsSync(receiptPath)) return null;
  try {
    const receipt = JSON.parse(fs.readFileSync(receiptPath, "utf8"));
    if (
      receipt.schemaVersion !== 1 ||
      receipt.postgresVersion !== manifest.postgresVersion ||
      receipt.target !== target ||
      receipt.sha256 !== manifest.targets[target].sha256
    )
      return null;
    const pair = toolPaths(installRoot, platform);
    if (!fs.existsSync(pair.pgDumpPath) || !fs.existsSync(pair.pgRestorePath))
      return null;
    if (
      receipt.pgDumpSha256 !== sha256FileSync(pair.pgDumpPath) ||
      receipt.pgRestoreSha256 !== sha256FileSync(pair.pgRestorePath)
    )
      return null;
    return validatePair(pair, manifest.expectedMajor, spawnImpl) ? pair : null;
  } catch (_error) {
    return null;
  }
}

async function installPinnedToolchain({
  repoRoot,
  target,
  platform,
  fetchImpl,
  spawnImpl,
  manifest = lock,
}) {
  const artifact = manifest.targets[target];
  if (!artifact)
    throw new Error(`unsupported platform target ${target || "<unknown>"}`);
  const baseRoot = path.join(
    repoRoot,
    "tmp",
    "toolchains",
    "postgresql",
    manifest.postgresVersion,
  );
  const installRoot = path.join(baseRoot, target);
  fs.mkdirSync(baseRoot, { recursive: true });
  const cached = readValidCache(
    installRoot,
    target,
    platform,
    spawnImpl,
    manifest,
  );
  if (cached) return { ...cached, source: "cache", target };

  const releaseLock = await acquireLock(`${installRoot}.lock`);
  const nonce = `${process.pid}-${crypto.randomBytes(6).toString("hex")}`;
  const archivePath = path.join(baseRoot, `${target}.${nonce}.tar.gz`);
  const stagingPath = path.join(baseRoot, `${target}.${nonce}.staging`);
  try {
    const afterLock = readValidCache(
      installRoot,
      target,
      platform,
      spawnImpl,
      manifest,
    );
    if (afterLock) return { ...afterLock, source: "cache", target };
    fs.mkdirSync(stagingPath);
    await downloadFile(artifact.url, archivePath, fetchImpl);
    const actualSha256 = await sha256File(archivePath);
    if (actualSha256 !== artifact.sha256)
      throw new Error(
        `checksum mismatch: expected ${artifact.sha256}, got ${actualSha256}`,
      );
    runTar(archivePath, stagingPath, spawnImpl);
    const pair = toolPaths(stagingPath, platform);
    if (!validatePair(pair, manifest.expectedMajor, spawnImpl))
      throw new Error(
        `downloaded tools are not PostgreSQL ${manifest.expectedMajor}`,
      );
    fs.writeFileSync(
      path.join(stagingPath, "receipt.json"),
      `${JSON.stringify({ schemaVersion: 1, postgresVersion: manifest.postgresVersion, target, url: artifact.url, sha256: artifact.sha256, pgDumpSha256: sha256FileSync(pair.pgDumpPath), pgRestoreSha256: sha256FileSync(pair.pgRestorePath), installedAt: new Date().toISOString() }, null, 2)}\n`,
      { flag: "wx" },
    );
    fs.rmSync(installRoot, { recursive: true, force: true });
    fs.renameSync(stagingPath, installRoot);
    return { ...toolPaths(installRoot, platform), source: "download", target };
  } finally {
    fs.rmSync(archivePath, { force: true });
    fs.rmSync(stagingPath, { recursive: true, force: true });
    releaseLock();
  }
}

function warning(message) {
  return `WARNING: PostgreSQL backup toolchain unavailable (${message}). API startup will continue; backup and recovery are disabled. Configure ${EXPLICIT_DUMP_ENV} and ${EXPLICIT_RESTORE_ENV} together to enable them.`;
}

async function resolvePostgresToolchain({
  repoRoot,
  sourceEnv = process.env,
  platform = process.platform,
  arch = process.arch,
  libc,
  fetchImpl = globalThis.fetch,
  spawnImpl = spawnSync,
  logImpl = () => {},
  manifest = lock,
} = {}) {
  const explicitDump = sourceEnv[EXPLICIT_DUMP_ENV];
  const explicitRestore = sourceEnv[EXPLICIT_RESTORE_ENV];
  if (explicitDump || explicitRestore) {
    if (!explicitDump || !explicitRestore) {
      logImpl(warning("explicit paths must be configured as a pair"));
      return null;
    }
    const pair = {
      pgDumpPath: path.resolve(explicitDump),
      pgRestorePath: path.resolve(explicitRestore),
    };
    if (!validatePair(pair, manifest.expectedMajor, spawnImpl)) {
      logImpl(
        warning(
          `explicit paths failed PostgreSQL ${manifest.expectedMajor} validation`,
        ),
      );
      return null;
    }
    return { ...pair, source: "explicit", target: null };
  }

  const target = detectTarget({ platform, arch, libc });
  if (target && manifest.targets[target]) {
    const installRoot = path.join(
      repoRoot,
      "tmp",
      "toolchains",
      "postgresql",
      manifest.postgresVersion,
      target,
    );
    const cached = readValidCache(
      installRoot,
      target,
      platform,
      spawnImpl,
      manifest,
    );
    if (cached) return { ...cached, source: "cache", target };
  }

  const systemPair = {
    pgDumpPath: pathCommand("pg_dump", sourceEnv, platform),
    pgRestorePath: pathCommand("pg_restore", sourceEnv, platform),
  };
  if (validatePair(systemPair, manifest.expectedMajor, spawnImpl))
    return { ...systemPair, source: "system", target };

  try {
    return await installPinnedToolchain({
      repoRoot,
      target,
      platform,
      fetchImpl,
      spawnImpl,
      manifest,
    });
  } catch (error) {
    logImpl(warning(`${target || `${platform}/${arch}`}: ${error.message}`));
    return null;
  }
}

function shouldResolveForAction(action, serviceKeys) {
  return START_ACTIONS.has(action) && serviceKeys.includes("api-server");
}

module.exports = {
  EXPLICIT_DUMP_ENV,
  EXPLICIT_RESTORE_ENV,
  assertSafeArchiveEntries,
  detectTarget,
  executableNames,
  installPinnedToolchain,
  resolvePostgresToolchain,
  shouldResolveForAction,
  validatePair,
};
